//! A program's own name, description and publisher, from its version resource.

use super::wide;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Strings {
    pub product: Option<String>,
    pub description: Option<String>,
    pub company: Option<String>,
}

impl Strings {
    /// What to call the program: its product name, else its description.
    pub fn name(&self) -> Option<String> {
        self.product.clone().or_else(|| self.description.clone())
    }
}

pub fn strings(path: &str) -> Strings {
    let file = wide(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None);
        if size == 0 {
            return Strings::default();
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(PCWSTR(file.as_ptr()), None, size, data.as_mut_ptr().cast()).is_err() {
            return Strings::default();
        }

        // The languages the resource lists, then the usual English ones.
        let mut languages = Vec::new();
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        let translation = wide("\\VarFileInfo\\Translation");
        if VerQueryValueW(data.as_ptr().cast(), PCWSTR(translation.as_ptr()), &mut ptr, &mut len).as_bool() && len >= 4
        {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            for pair in pairs.as_chunks::<2>().0 {
                languages.push(format!("{:04x}{:04x}", pair[0], pair[1]));
            }
        }
        languages.push("040904b0".to_string());
        languages.push("040904e4".to_string());

        let read = |field: &str| -> Option<String> {
            for language in &languages {
                let key = wide(&format!("\\StringFileInfo\\{language}\\{field}"));
                let mut value = std::ptr::null_mut();
                let mut chars = 0u32;
                if VerQueryValueW(data.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut value, &mut chars).as_bool()
                    && chars > 1
                {
                    let text =
                        String::from_utf16_lossy(std::slice::from_raw_parts(value as *const u16, chars as usize - 1));
                    let text = text.trim_matches(char::from(0)).trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
            None
        };
        Strings {
            product: read("ProductName"),
            description: read("FileDescription"),
            company: read("CompanyName"),
        }
    }
}
