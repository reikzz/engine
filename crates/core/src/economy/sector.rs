use crate::economy::RecipeId;
use crate::registry::IdentifyableDef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SectorKind {
    Energy = 0,
    Extraction = 1,
    Agro = 2,
    HeavyIndustry = 3,
    Petrochemistry = 4,
    Automotive = 5,
    HighTech = 6,
    Consumer = 7,
    MilitaryComplex = 8,
    Construction = 9,
    Financial = 10,
    InformationTechnology = 11,
    RealEstate = 12,
}

impl SectorKind {
    #[inline]
    pub fn loc_key(self) -> &'static str {
        match self {
            Self::Energy => "sector_energy",
            Self::Extraction => "sector_extraction",
            Self::Agro => "sector_agro",
            Self::HeavyIndustry => "sector_heavy_industry",
            Self::Petrochemistry => "sector_petrochemistry",
            Self::Automotive => "sector_automotive",
            Self::HighTech => "sector_high_tech",
            Self::Consumer => "sector_consumer",
            Self::MilitaryComplex => "sector_military_complex",
            Self::Construction => "sector_construction",
            Self::Financial => "sector_financial",
            Self::InformationTechnology => "sector_information_technology",
            Self::RealEstate => "sector_real_estate",
        }
    }

    #[inline]
    pub fn infrastructure_weight(self) -> f64 {
        match self {
            Self::Extraction | Self::HeavyIndustry | Self::Petrochemistry => 1.2,
            Self::Agro | Self::Construction | Self::Automotive => 0.8,
            Self::Consumer | Self::Energy => 0.5,
            Self::HighTech | Self::MilitaryComplex => 0.3,
            Self::Financial | Self::InformationTechnology | Self::RealEstate => 0.05,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Sector {
    pub kind: SectorKind,
    #[serde(default)]
    pub available_recipes: Vec<RecipeId>,
}

impl IdentifyableDef for Sector {
    type Id = SectorKind;

    fn key(&self) -> &str {
        self.kind.loc_key()
    }

    fn from_index(index: usize) -> Self::Id {
        match index {
            0 => SectorKind::Energy,
            1 => SectorKind::Extraction,
            2 => SectorKind::Agro,
            3 => SectorKind::HeavyIndustry,
            4 => SectorKind::Petrochemistry,
            5 => SectorKind::Automotive,
            6 => SectorKind::HighTech,
            7 => SectorKind::Consumer,
            8 => SectorKind::MilitaryComplex,
            9 => SectorKind::Construction,
            10 => SectorKind::Financial,
            11 => SectorKind::InformationTechnology,
            12 => SectorKind::RealEstate,
            _ => panic!("Invalid index for SectorKind: {}", index),
        }
    }

    fn to_index(id: Self::Id) -> usize {
        id as usize
    }
}
