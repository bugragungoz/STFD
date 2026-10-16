//! A program's icon as a PNG, for the window to show next to its name.

use super::wide;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HGDIOBJ,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

/// The program's first icon at `size` pixels, as PNG bytes.
pub fn png(path: &str, size: u32) -> Option<Vec<u8>> {
    let file = wide(path);
    let mut icon = HICON::default();
    unsafe {
        // The low word asks for the large icon's size.
        SHDefExtractIconW(
            PCWSTR(file.as_ptr()),
            0,
            0,
            Some(&mut icon as *mut HICON),
            None,
            size & 0xffff,
        )
        .ok()
        .ok()?;
        if icon.is_invalid() {
            return None;
        }
        let pixels = rgba(icon, size);
        let _ = DestroyIcon(icon);
        encode(&pixels?, size)
    }
}

/// The icon's pixels as RGBA, top row first.
unsafe fn rgba(icon: HICON, size: u32) -> Option<Vec<u8>> {
    let mut info = ICONINFO::default();
    GetIconInfo(icon, &mut info).ok()?;
    let dc = CreateCompatibleDC(None);
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            // Negative: rows from the top.
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; (size * size * 4) as usize];
    let lines = GetDIBits(
        dc,
        info.hbmColor,
        0,
        size,
        Some(bgra.as_mut_ptr().cast()),
        &mut header,
        DIB_RGB_COLORS,
    );
    // Icons without an alpha channel mark transparency in the mask instead.
    let mut mask = vec![0u8; (size * size * 4) as usize];
    let mut mask_header = header;
    let mask_lines = GetDIBits(
        dc,
        info.hbmMask,
        0,
        size,
        Some(mask.as_mut_ptr().cast()),
        &mut mask_header,
        DIB_RGB_COLORS,
    );
    let _ = DeleteDC(dc);
    if !info.hbmColor.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
    }
    if !info.hbmMask.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
    }
    if lines == 0 {
        return None;
    }
    let has_alpha = bgra.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    let mut out = Vec::with_capacity(bgra.len());
    for (i, p) in bgra.as_chunks::<4>().0.iter().enumerate() {
        let alpha = if has_alpha {
            p[3]
        } else if mask_lines > 0 && mask[i * 4] != 0 {
            0
        } else {
            255
        };
        out.extend_from_slice(&[p[2], p[1], p[0], alpha]);
    }
    Some(out)
}

fn encode(rgba: &[u8], size: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}
