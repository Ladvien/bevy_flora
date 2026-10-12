//! The textures bundled with the crate (CC0 scans, see `src/textures/CREDITS.md`) as images.
//!
//! [`FloraTextures`] decodes each scan the first time a tree asks for it and keeps it, so a
//! forest of oaks shares one pair of images. Without the `textures` feature nothing is bundled
//! and every scan resolves to `None`: wood is plain colour.

use std::collections::HashMap;

use bevy::asset::{Assets, Handle};
use bevy::color::{Color, LinearRgba};
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::image::Image;

use crate::params::{BarkScan, TreeParams};

/// Most a material's base colour may be brightened by dividing out a scan's average colour
/// (per linear channel): a pale preset on a dark scan, such as white birch, must not overflow.
const MAX_TINT: f32 = 8.0;

/// A decoded bark scan: its images and average colour.
#[derive(Clone, Debug)]
pub struct BarkTexture {
    /// Which scan this is.
    pub scan: BarkScan,
    /// The colour map (sRGB, repeating, with mipmaps).
    pub color: Handle<Image>,
    /// The normal map (tangent space, OpenGL convention, repeating, with mipmaps).
    pub normal: Handle<Image>,
    /// The colour map's average, in linear light.
    pub mean: LinearRgba,
}

impl BarkTexture {
    /// The base colour a material needs for the scan to average out to `preset`: `preset`
    /// divided by the scan's mean, per linear channel and at most 8 times brighter. Multiplying
    /// it by the colour map gives wood whose average colour is `preset`, so far trees, impostors
    /// and untextured trees keep their colour.
    pub fn base_color(&self, preset: Color) -> Color {
        let preset = preset.to_linear();
        let divide = |preset: f32, mean: f32| (preset / mean.max(1.0e-3)).min(MAX_TINT);
        Color::linear_rgba(
            divide(preset.red, self.mean.red),
            divide(preset.green, self.mean.green),
            divide(preset.blue, self.mean.blue),
            preset.alpha,
        )
    }
}

/// The bundled textures, decoded on first use. Resolving a scan needs the image assets; where
/// there are none (a headless app) or the scan cannot be decoded, trees are plain colour.
#[derive(Resource, Default)]
pub struct FloraTextures {
    bark: HashMap<BarkScan, Option<BarkTexture>>,
}

impl FloraTextures {
    /// The images of `scan`, decoded and added to `images` on the first call. `None` without the
    /// `textures` feature, or when the scan fails to decode (logged once).
    pub fn bark(&mut self, images: &mut Assets<Image>, scan: BarkScan) -> Option<BarkTexture> {
        self.bark.entry(scan).or_insert_with(|| load_bark(images, scan)).clone()
    }
}

/// [`FloraTextures`] and the image assets together, for systems that resolve a tree's bark: the
/// plugin's systems take it. Not for building by hand.
#[derive(SystemParam)]
pub struct BarkLibrary<'w> {
    textures: ResMut<'w, FloraTextures>,
    images: Option<ResMut<'w, Assets<Image>>>,
}

impl BarkLibrary<'_> {
    /// The bark scan `params` ask for, or `None` for plain colour.
    pub(crate) fn of(&mut self, params: &TreeParams) -> Option<BarkTexture> {
        let scan = params.bark.scan?;
        let images = self.images.as_deref_mut()?;
        self.textures.bark(images, scan)
    }
}

#[cfg(feature = "textures")]
fn load_bark(images: &mut Assets<Image>, scan: BarkScan) -> Option<BarkTexture> {
    bundled::load_bark(images, scan)
}

#[cfg(not(feature = "textures"))]
fn load_bark(_images: &mut Assets<Image>, _scan: BarkScan) -> Option<BarkTexture> {
    None
}

/// Decoding the compiled-in files.
#[cfg(feature = "textures")]
mod bundled {
    use bevy::asset::{Assets, RenderAssetUsages};
    use bevy::color::LinearRgba;
    use bevy::image::{
        CompressedImageFormats, Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor,
        ImageType,
    };
    use bevy::log::warn;
    use bevy::render::render_resource::TextureFormat;

    use super::BarkTexture;
    use crate::impostor::{MIP_LEVELS, srgb_to_linear, with_mip_chain};
    use crate::params::BarkScan;

    /// Decode, mip and add the colour and normal map of `scan`.
    pub(super) fn load_bark(images: &mut Assets<Image>, scan: BarkScan) -> Option<BarkTexture> {
        let (color_bytes, normal_bytes) = bark_bytes(scan);
        let name = bark_name(scan);
        let fail = |what: &str, error: &str| {
            warn!("bevy_flora: bark texture {name} ({what}) cannot be used ({error}); its trees are plain colour");
        };
        let mut color = decode_jpeg(color_bytes, true).inspect_err(|e| fail("colour", e)).ok()?;
        let mut normal = decode_jpeg(normal_bytes, false).inspect_err(|e| fail("normal", e)).ok()?;
        let mean = mean_linear(&color).or_else(|| {
            fail("colour", "no pixel data");
            None
        })?;
        let chain = |image: &mut Image, mips: fn(&[u8], u32, u32) -> Vec<u8>| -> Result<(), String> {
            let (w, h) = (image.width(), image.height());
            let step = 1u32 << (MIP_LEVELS - 1);
            if w % step != 0 || h % step != 0 {
                return Err(format!("{w}x{h} is not a multiple of {step} pixels"));
            }
            let base = image.data.as_deref().ok_or("no pixel data")?;
            image.data = Some(mips(base, w, h));
            image.texture_descriptor.mip_level_count = MIP_LEVELS;
            Ok(())
        };
        chain(&mut color, with_mip_chain).inspect_err(|e| fail("colour", e)).ok()?;
        chain(&mut normal, normal_mip_chain).inspect_err(|e| fail("normal", e)).ok()?;
        Some(BarkTexture { scan, color: images.add(color), normal: images.add(normal), mean })
    }

    /// The sampler of the wood and leaf textures: they tile, so repeat; filtered, with the
    /// anisotropy a trunk seen along its length needs.
    fn sampler() -> ImageSampler {
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        })
    }

    /// A JPEG as an RGBA8 image, sRGB or linear, without mipmaps.
    fn decode_jpeg(bytes: &[u8], srgb: bool) -> Result<Image, String> {
        let image = Image::from_buffer(
            bytes,
            ImageType::Extension("jpg"),
            CompressedImageFormats::NONE,
            srgb,
            sampler(),
            RenderAssetUsages::RENDER_WORLD,
        )
        .map_err(|error| error.to_string())?;
        let format = if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
        if image.texture_descriptor.format == format {
            return Ok(image);
        }
        let from = image.texture_descriptor.format;
        image.convert(format).ok_or_else(|| format!("{from:?} cannot be converted to {format:?}"))
    }

    /// The average linear colour of an RGBA8 sRGB image, or `None` without pixel data.
    fn mean_linear(image: &Image) -> Option<LinearRgba> {
        let data = image.data.as_deref()?;
        let mut sum = [0.0f64; 3];
        for texel in data.as_chunks::<4>().0 {
            for (total, &byte) in sum.iter_mut().zip(texel) {
                *total += f64::from(srgb_to_linear(byte));
            }
        }
        let count = (data.len() / 4).max(1) as f64;
        Some(LinearRgba::rgb((sum[0] / count) as f32, (sum[1] / count) as f32, (sum[2] / count) as f32))
    }

    /// `base` (RGBA8 tangent-space normals, `width` x `height`) followed by its smaller levels,
    /// [`MIP_LEVELS`] in all. Each texel averages the four below it as vectors and renormalises,
    /// so distant bark smooths to a flat surface instead of a darker one; alpha is 255.
    pub(super) fn normal_mip_chain(base: &[u8], width: u32, height: u32) -> Vec<u8> {
        let decode = |byte: u8| f32::from(byte) / 127.5 - 1.0;
        let encode = |value: f32| ((value * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        let mut chain = base.to_vec();
        let (mut w, mut h) = (width as usize, height as usize);
        let mut from = 0;
        for _ in 1..MIP_LEVELS {
            let (nw, nh) = (w / 2, h / 2);
            let mut level = Vec::with_capacity(nw * nh * 4);
            for y in 0..nh {
                for x in 0..nw {
                    let mut sum = [0.0f32; 3];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let at = from + ((2 * y + dy) * w + 2 * x + dx) * 4;
                        let n = [decode(chain[at]), decode(chain[at + 1]), decode(chain[at + 2])];
                        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1.0e-6);
                        for (total, value) in sum.iter_mut().zip(n) {
                            *total += value / length;
                        }
                    }
                    let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt().max(1.0e-6);
                    level.extend(sum.map(|value| encode(value / length)));
                    level.push(255);
                }
            }
            from = chain.len();
            chain.extend_from_slice(&level);
            (w, h) = (nw, nh);
        }
        chain
    }

    /// The file stem of a scan in `src/textures/bark/`.
    fn bark_name(scan: BarkScan) -> &'static str {
        match scan {
            BarkScan::Oak => "oak",
            BarkScan::Pine => "pine",
            BarkScan::Willow => "willow",
            BarkScan::Birch => "birch",
            BarkScan::Palm => "palm",
            BarkScan::Cedar => "cedar",
            BarkScan::Zelkova => "zelkova",
            BarkScan::Maple => "maple",
            BarkScan::KnottedPine => "knotted_pine",
            BarkScan::Sakura => "sakura",
            BarkScan::PalmTree => "palm_tree",
            BarkScan::Hackberry => "hackberry",
            BarkScan::Brown => "brown",
            BarkScan::Metasequoia => "metasequoia",
        }
    }

    /// A scan's colour and normal map files, compiled in.
    fn bark_bytes(scan: BarkScan) -> (&'static [u8], &'static [u8]) {
        macro_rules! files {
            ($name:literal) => {
                (
                    include_bytes!(concat!("../textures/bark/", $name, "_color.jpg")).as_slice(),
                    include_bytes!(concat!("../textures/bark/", $name, "_normal.jpg")).as_slice(),
                )
            };
        }
        match scan {
            BarkScan::Oak => files!("oak"),
            BarkScan::Pine => files!("pine"),
            BarkScan::Willow => files!("willow"),
            BarkScan::Birch => files!("birch"),
            BarkScan::Palm => files!("palm"),
            BarkScan::Cedar => files!("cedar"),
            BarkScan::Zelkova => files!("zelkova"),
            BarkScan::Maple => files!("maple"),
            BarkScan::KnottedPine => files!("knotted_pine"),
            BarkScan::Sakura => files!("sakura"),
            BarkScan::PalmTree => files!("palm_tree"),
            BarkScan::Hackberry => files!("hackberry"),
            BarkScan::Brown => files!("brown"),
            BarkScan::Metasequoia => files!("metasequoia"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "textures")]
    use crate::impostor::MIP_LEVELS;
    #[cfg(feature = "textures")]
    use bevy::render::render_resource::TextureFormat;

    #[cfg(feature = "textures")]
    #[test]
    fn a_normal_mip_chain_averages_as_vectors() {
        // 32x32 normals tilted left in the left half and right in the right half, equally.
        let (w, h) = (32usize, 32usize);
        let tilt = |x: usize| if x < w / 2 { [64u8, 128, 220, 255] } else { [192u8, 128, 220, 255] };
        let base: Vec<u8> = (0..w * h).flat_map(|i| tilt(i % w)).collect();
        let chain = bundled::normal_mip_chain(&base, w as u32, h as u32);
        let texels: usize = (0..MIP_LEVELS).map(|l| (w >> l) * (h >> l)).sum();
        assert_eq!(chain.len(), texels * 4);
        // The 1x1 level (the last texel) is the flat normal: x and y centred, z the strongest.
        let last = &chain[chain.len() - 4..];
        assert!((i32::from(last[0]) - 128).abs() <= 1 && (i32::from(last[1]) - 128).abs() <= 1, "{last:?}");
        assert!(last[2] > 220 && last[3] == 255, "{last:?}");
    }

    #[cfg(feature = "textures")]
    #[test]
    fn every_bundled_bark_scan_decodes_to_filterable_mipmapped_images() {
        let mut images = Assets::<Image>::default();
        let mut textures = FloraTextures::default();
        for scan in BarkScan::ALL {
            let bark = textures.bark(&mut images, scan).unwrap_or_else(|| panic!("{scan:?} did not load"));
            for (handle, srgb) in [(&bark.color, true), (&bark.normal, false)] {
                let image = images.get(handle).expect("the image was added");
                let format = if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
                assert_eq!(image.texture_descriptor.format, format, "{scan:?}");
                assert_eq!(image.texture_descriptor.mip_level_count, MIP_LEVELS, "{scan:?}");
                let texels: usize =
                    (0..MIP_LEVELS).map(|l| (image.width() as usize >> l) * (image.height() as usize >> l)).sum();
                assert_eq!(image.data.as_ref().map(Vec::len), Some(texels * 4), "{scan:?}: mip chain length");
            }
            let normal = images.get(&bark.normal).expect("normal");
            let data = normal.data.as_ref().expect("data");
            let base = &data[..(normal.width() * normal.height()) as usize * 4];
            let mean_z: f32 =
                base.as_chunks::<4>().0.iter().map(|t| f32::from(t[2]) / 255.0 * 2.0 - 1.0).sum::<f32>() / (base.len() / 4) as f32;
            assert!(mean_z > 0.5, "{scan:?}: normals do not point out of the surface (mean z {mean_z})");
            for channel in [bark.mean.red, bark.mean.green, bark.mean.blue] {
                assert!((0.005..0.9).contains(&channel), "{scan:?}: implausible mean {:?}", bark.mean);
            }
        }
    }

    #[test]
    fn dividing_out_the_mean_keeps_the_preset_colour_on_average() {
        let bark = BarkTexture {
            scan: BarkScan::Oak,
            color: Handle::default(),
            normal: Handle::default(),
            mean: LinearRgba::rgb(0.2, 0.1, 0.005),
        };
        let preset = Color::linear_rgb(0.1, 0.1, 0.1);
        let base = bark.base_color(preset).to_linear();
        // A texture averaging `mean` times this base averages the preset.
        assert!((base.red * 0.2 - 0.1).abs() < 1e-5);
        assert!((base.green * 0.1 - 0.1).abs() < 1e-5);
        // A dark channel is clamped rather than blown up.
        assert_eq!(base.blue, MAX_TINT);
    }
}
