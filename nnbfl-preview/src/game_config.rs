#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Language {
    #[default]
    /// US English
    USen,

    /// EU English
    EUen,

    /// Japanese
    JPja,

    /// Simplified Chinese
    CNzh,

    /// Traditional Chinese
    TWzh,

    /// Korean
    KRko,

    /// EU French
    EUfr,

    /// US French
    USfr,

    /// German
    EUde,

    /// EU Spanish
    EUes,

    /// US Spanish
    USes,

    /// Italian
    EUit,

    /// Dutch
    EUnl,

    /// Russian
    EUru,
}

impl Language {
    pub const ALL: &'static [Language] = &[
        Language::USen,
        Language::EUen,
        Language::JPja,
        Language::CNzh,
        Language::TWzh,
        Language::KRko,
        Language::EUfr,
        Language::USfr,
        Language::EUde,
        Language::EUes,
        Language::USes,
        Language::EUit,
        Language::EUnl,
        Language::EUru,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::USen => "English (US)",
            Self::EUen => "English (EU)",
            Self::JPja => "Japanese (日本語)",
            Self::CNzh => "Simplified Chinese (简体中文)",
            Self::TWzh => "Traditional Chinese (繁體中文)",
            Self::KRko => "Korean (한국어)",
            Self::EUfr => "French (EU - Français)",
            Self::USfr => "French (US - Français)",
            Self::EUde => "German (Deutsch)",
            Self::EUes => "Spanish (EU - Español)",
            Self::USes => "Spanish (US - Español)",
            Self::EUit => "Italian (Italiano)",
            Self::EUnl => "Dutch (Nederlands)",
            Self::EUru => "Russian (Русский)",
        }
    }

    pub fn code_str(&self) -> &'static str {
        match self {
            Self::USen => "USen",
            Self::EUen => "EUen",
            Self::JPja => "JPja",
            Self::CNzh => "CNzh",
            Self::TWzh => "TWzh",
            Self::KRko => "KRko",
            Self::EUfr => "EUfr",
            Self::USfr => "USfr",
            Self::EUde => "EUde",
            Self::EUes => "EUes",
            Self::USes => "USes",
            Self::EUit => "EUit",
            Self::EUnl => "EUnl",
            Self::EUru => "EUru",
        }
    }

    pub fn region_short(&self) -> &'static str {
        match self {
            Self::USen | Self::USfr | Self::USes => "US",
            Self::EUen
            | Self::EUfr
            | Self::EUde
            | Self::EUes
            | Self::EUit
            | Self::EUnl
            | Self::EUru => "EU",
            Self::JPja => "JP",
            Self::CNzh => "CN",
            Self::TWzh => "TW",
            Self::KRko => "KR",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}
