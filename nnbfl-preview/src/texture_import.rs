use std::path::Path;

use tomolib::formats::bntx::{
    Texture,
    image::{RgbaImage, can_encode, encode_mips_swizzled, png_to_rgba},
};

pub fn png_size(path: &Path) -> Result<(u32, u32), String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;

    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return Err(format!("{} isn't a PNG", path.display()));
    }

    let size =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);

    Ok((size(16), size(20)))
}

pub fn load_png(path: &Path, size: (u32, u32), resize: bool) -> Result<RgbaImage, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;

    let image =
        png_to_rgba(&bytes).map_err(|e| format!("Couldn't decode {}: {e}", path.display()))?;

    if (image.width, image.height) == size {
        Ok(image)
    } else if resize {
        Ok(resize_box(&image, size.0, size.1))
    } else {
        Err(format!(
            "{} is {}x{}, the texture is {}x{}",
            path.display(),
            image.width,
            image.height,
            size.0,
            size.1
        ))
    }
}

pub fn resize_box(src: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (src_w, src_h) = (src.width as usize, src.height as usize);
    let (dst_w, dst_h) = (width as usize, height as usize);

    let mut data = vec![0u8; dst_w * dst_h * 4];

    for y in 0..dst_h {
        let y0 = y * src_h / dst_h;
        let y1 = ((y + 1) * src_h / dst_h).max(y0 + 1);

        for x in 0..dst_w {
            let x0 = x * src_w / dst_w;
            let x1 = ((x + 1) * src_w / dst_w).max(x0 + 1);
            let mut sum = [0u32; 4];

            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * src_w + sx) * 4;

                    for (total, value) in sum.iter_mut().zip(&src.data[i..i + 4]) {
                        *total += u32::from(*value);
                    }
                }
            }

            let count = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (y * dst_w + x) * 4;

            for (out, total) in data[o..o + 4].iter_mut().zip(sum) {
                *out = (total / count) as u8;
            }
        }
    }

    RgbaImage {
        width,
        height,
        data,
    }
}

pub fn encode_like(image: &RgbaImage, template: &Texture, name: &str) -> Result<Texture, String> {
    if !can_encode(&template.info) {
        return Err(format!(
            "{} textures can't be encoded",
            template.info.format.name()
        ));
    }

    let image_data = encode_mips_swizzled(image, template)
        .map_err(|e| format!("Encoding as {} failed: {e}", template.info.format.name()))?;

    Ok(Texture {
        name: name.to_string(),
        info: template.info.clone(),
        mip_offsets: template.mip_offsets.clone(),
        user_data: Vec::new(),
        image_data,
    })
}

pub fn texture_name_error(name: &str, taken: impl Fn(&str) -> bool) -> Option<String> {
    if name.is_empty() {
        Some("The name is empty".into())
    } else if !name.bytes().all(|b| b.is_ascii_graphic()) {
        Some("Names are ASCII without spaces".into())
    } else if taken(name) {
        Some(format!("A texture named {name} is already loaded"))
    } else {
        None
    }
}
