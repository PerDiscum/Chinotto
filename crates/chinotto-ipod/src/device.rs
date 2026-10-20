//! Recognising an iPod's folder and reading what it says about itself.
//!
//! `iPod_Control/Device/SysInfo` is a plain `Key: value` text file. The
//! interesting keys are `ModelNumStr` (e.g. `xC297`; the first character is a
//! region/prefix letter and is ignored) and `FirewireGuid` (needed later for
//! the Classic's database checksum).

use std::path::{Path, PathBuf};

/// Paths inside an iPod root.
pub struct Layout {
    pub root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn control(&self) -> PathBuf {
        self.root.join("iPod_Control")
    }
    pub fn itunesdb(&self) -> PathBuf {
        self.control().join("iTunes/iTunesDB")
    }
    pub fn artworkdb(&self) -> PathBuf {
        self.control().join("Artwork/ArtworkDB")
    }
    pub fn artwork_dir(&self) -> PathBuf {
        self.control().join("Artwork")
    }
    pub fn sysinfo(&self) -> PathBuf {
        self.control().join("Device/SysInfo")
    }
}

/// True when `root` looks like the top of an iPod (has an iTunesDB).
pub fn is_ipod_root(root: &Path) -> bool {
    Layout::new(root).itunesdb().is_file()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SysInfo {
    pub model_number: Option<String>,
    pub firewire_guid: Option<String>,
    pub serial: Option<String>,
}

impl SysInfo {
    pub fn parse(text: &str) -> Self {
        let mut s = SysInfo::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else { continue };
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            match k.trim() {
                "ModelNumStr" => s.model_number = Some(v.to_string()),
                "FirewireGuid" => s.firewire_guid = Some(v.to_string()),
                "pszSerialNumber" => s.serial = Some(v.to_string()),
                _ => {}
            }
        }
        s
    }

    pub fn read(root: &Path) -> Option<Self> {
        std::fs::read_to_string(Layout::new(root).sysinfo()).ok().map(|t| Self::parse(&t))
    }

    /// Human-readable model, when the model number is one we know.
    pub fn model_name(&self) -> Option<&'static str> {
        model_name(self.model_number.as_deref()?)
    }
}

/// Model names for the classic iPods, keyed by model number without the
/// leading prefix letter. Capacities are left out on purpose: modded iPods
/// often hold far more than the label says.
pub fn model_name(model_number: &str) -> Option<&'static str> {
    let key = model_number.get(1..).unwrap_or("");
    Some(match key {
        "B029" | "B147" | "B145" | "B150" => "iPod classic (6th generation)",
        "B562" | "B565" => "iPod classic (6th generation, late 2008)",
        "C293" | "C297" => "iPod classic (7th generation)",
        "A002" | "A146" | "A003" | "A147" | "A444" | "A446" | "A448" | "A450" | "A664" => "iPod with video (5th generation)",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sysinfo() {
        let s = SysInfo::parse("BoardHwName: iPod Q98\nModelNumStr: xC297\nFirewireGuid: 0x000A27001B2C3D4E\nVisibleBuildID: 0x02208000\n");
        assert_eq!(s.model_number.as_deref(), Some("xC297"));
        assert_eq!(s.firewire_guid.as_deref(), Some("0x000A27001B2C3D4E"));
        assert_eq!(s.model_name(), Some("iPod classic (7th generation)"));
    }
}
