use crate::geometry::Rect;

pub const MAX_BLUR: f32 = 120.0;

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub enum ColorVision {
    #[default]
    Typical,
    Protanopia,
    Deuteranopia,
    Tritanopia,
    Achromatopsia,
}

impl ColorVision {
    pub const ALL: [(&'static str, Self); 5] = [
        ("None", Self::Typical),
        ("Protanopia", Self::Protanopia),
        ("Deuteranopia", Self::Deuteranopia),
        ("Tritanopia", Self::Tritanopia),
        ("Achromatopsia", Self::Achromatopsia),
    ];

    pub fn matrix(self) -> [[f32; 3]; 3] {
        match self {
            Self::Typical => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Self::Protanopia => [
                [0.152_286, 1.052_583, -0.204_868],
                [0.114_503, 0.786_281, 0.099_216],
                [-0.003_882, -0.048_116, 1.051_998],
            ],
            Self::Deuteranopia => [
                [0.367_322, 0.860_646, -0.227_968],
                [0.280_085, 0.672_501, 0.047_413],
                [-0.011_820, 0.042_940, 0.968_881],
            ],
            Self::Tritanopia => [
                [1.255_528, -0.076_749, -0.178_779],
                [-0.078_411, 0.930_809, 0.147_602],
                [0.004_733, 0.691_367, 0.303_900],
            ],
            Self::Achromatopsia => [
                [0.212_6, 0.715_2, 0.072_2],
                [0.212_6, 0.715_2, 0.072_2],
                [0.212_6, 0.715_2, 0.072_2],
            ],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Filter {
    pub region: Rect,
    pub blur: f32,
    pub contrast: f32,
    pub vision: ColorVision,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            region: Rect::EVERYTHING,
            blur: 0.0,
            contrast: 1.0,
            vision: ColorVision::Typical,
        }
    }
}

impl Filter {
    pub fn changes_nothing(&self) -> bool {
        self.blur <= 0.0 && self.contrast >= 1.0 && self.vision == ColorVision::Typical
    }
}
