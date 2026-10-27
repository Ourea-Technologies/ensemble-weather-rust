//! String enums of the API, each with an `Unknown` case for values added later.

/// Defines a string enum that keeps unknown values.
///
/// Every generated enum is `#[non_exhaustive]`, (de)serialises as its wire
/// string, and parses any string it does not know into `Unknown(raw)`, so a
/// value the API adds later is never an error (BEHAVIOR 1.5).
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $wire:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this version of the SDK does not know, kept verbatim.
            Unknown(String),
        }

        impl $name {
            /// The value as the API spells it.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Unknown(raw) => raw.as_str(),
                }
            }

            /// Whether this is a value the SDK does not know.
            #[must_use]
            pub fn is_unknown(&self) -> bool {
                matches!(self, Self::Unknown(_))
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::convert::From<&str> for $name {
            fn from(raw: &str) -> Self {
                match raw {
                    $( $wire => Self::$variant, )+
                    other => Self::Unknown(other.to_owned()),
                }
            }
        }

        impl ::std::convert::From<String> for $name {
            fn from(raw: String) -> Self {
                match Self::from(raw.as_str()) {
                    Self::Unknown(_) => Self::Unknown(raw),
                    known => known,
                }
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = ::std::convert::Infallible;
            fn from_str(raw: &str) -> ::std::result::Result<Self, Self::Err> {
                Ok(Self::from(raw))
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> ::std::result::Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> ::std::result::Result<Self, D::Error> {
                let raw = <::std::borrow::Cow<'de, str> as ::serde::Deserialize>::deserialize(d)?;
                Ok(Self::from(raw.into_owned()))
            }
        }
    };
}

pub(crate) use string_enum;

string_enum! {
    /// The kind of a named location.
    ///
    /// ```
    /// use ensemble_weather::LocationType;
    /// assert_eq!("ski_area".parse::<LocationType>().unwrap(), LocationType::SkiArea);
    /// assert!(LocationType::from("a_type_added_later").is_unknown());
    /// ```
    pub enum LocationType {
        /// An airport.
        Airport => "airport",
        /// Backcountry terrain.
        Backcountry => "backcountry",
        /// A beach or stretch of coast.
        BeachCoast => "beach_coast",
        /// A campground.
        Campground => "campground",
        /// A city or town.
        CityTown => "city_town",
        /// A climbing area.
        ClimbingArea => "climbing_area",
        /// A coastal point.
        CoastalPoint => "coastal_point",
        /// An event venue.
        EventVenue => "event_venue",
        /// A fire lookout.
        FireLookout => "fire_lookout",
        /// Fishing or river access.
        FishingRiverAccess => "fishing_river_access",
        /// A golf course.
        GolfCourse => "golf_course",
        /// A hot spring.
        HotSpring => "hot_spring",
        /// A lake.
        Lake => "lake",
        /// A mountain pass.
        MountainPass => "mountain_pass",
        /// A national park.
        NationalPark => "national_park",
        /// An observation station.
        ObservationStation => "observation_station",
        /// Any other kind.
        Other => "other",
        /// A park.
        Park => "park",
        /// A recreation area.
        RecreationArea => "recreation_area",
        /// A reservoir.
        ReservoirLake => "reservoir_lake",
        /// A scenic or tourist spot.
        ScenicTourist => "scenic_tourist",
        /// A ski area.
        SkiArea => "ski_area",
        /// A state park.
        StatePark => "state_park",
        /// A summit.
        Summit => "summit",
        /// A trailhead.
        Trailhead => "trailhead",
        /// A volcano.
        Volcano => "volcano",
        /// A waterfall.
        Waterfall => "waterfall",
        /// A winery or agritourism site.
        WineAgritourism => "wine_agritourism",
    }
}

string_enum! {
    /// A weather condition code, for an hour or a day.
    ///
    /// ```
    /// use ensemble_weather::Condition;
    /// assert_eq!(Condition::from("snow").as_str(), "snow");
    /// ```
    pub enum Condition {
        /// Sunny.
        Sunny => "sunny",
        /// Mostly sunny.
        MostlySunny => "mostly_sunny",
        /// Partly cloudy.
        PartlyCloudy => "partly_cloudy",
        /// Mostly cloudy.
        MostlyCloudy => "mostly_cloudy",
        /// Cloudy.
        Cloudy => "cloudy",
        /// Clear (night).
        Clear => "clear",
        /// Mostly clear (night).
        MostlyClear => "mostly_clear",
        /// A slight chance of showers.
        SlightChanceShowers => "slight_chance_showers",
        /// A chance of showers.
        ChanceShowers => "chance_showers",
        /// A chance of rain.
        ChanceRain => "chance_rain",
        /// Showers likely.
        ShowersLikely => "showers_likely",
        /// Rain likely.
        RainLikely => "rain_likely",
        /// Showers.
        Showers => "showers",
        /// Rain.
        Rain => "rain",
        /// Light rain.
        LightRain => "light_rain",
        /// Heavy rain.
        HeavyRain => "heavy_rain",
        /// A slight chance of a rain and snow mix.
        SlightChanceRainSnowMix => "slight_chance_rain_snow_mix",
        /// A chance of a rain and snow mix.
        ChanceRainSnowMix => "chance_rain_snow_mix",
        /// A rain and snow mix likely.
        RainSnowMixLikely => "rain_snow_mix_likely",
        /// A rain and snow mix.
        RainSnowMix => "rain_snow_mix",
        /// A slight chance of snow.
        SlightChanceSnow => "slight_chance_snow",
        /// A chance of snow.
        ChanceSnow => "chance_snow",
        /// Snow likely.
        SnowLikely => "snow_likely",
        /// Snow.
        Snow => "snow",
        /// A slight chance of freezing rain.
        SlightChanceFreezingRain => "slight_chance_freezing_rain",
        /// A chance of freezing rain.
        ChanceFreezingRain => "chance_freezing_rain",
        /// Freezing rain likely.
        FreezingRainLikely => "freezing_rain_likely",
        /// Freezing rain.
        FreezingRain => "freezing_rain",
    }
}

string_enum! {
    /// A precipitation type.
    ///
    /// ```
    /// use ensemble_weather::PrecipKind;
    /// assert_eq!(PrecipKind::from("snow"), PrecipKind::Snow);
    /// ```
    pub enum PrecipKind {
        /// No precipitation.
        None => "none",
        /// Rain.
        Rain => "rain",
        /// Snow.
        Snow => "snow",
        /// A rain and snow mix.
        RainSnowMix => "rain_snow_mix",
        /// Freezing rain.
        FreezingRain => "freezing_rain",
    }
}

string_enum! {
    /// A live input of the nowcast layer.
    ///
    /// ```
    /// use ensemble_weather::NowcastSource;
    /// assert_eq!(NowcastSource::Radar.as_str(), "radar");
    /// ```
    pub enum NowcastSource {
        /// Surface observations.
        Observation => "observation",
        /// Radar.
        Radar => "radar",
        /// Satellite cloud.
        Cloud => "cloud",
        /// The nearest sensors.
        NearestSensors => "nearest_sensors",
    }
}

string_enum! {
    /// Which per-member values a forecast carries.
    ///
    /// ```
    /// use ensemble_weather::MembersView;
    /// assert_eq!(MembersView::from("plume"), MembersView::Plume);
    /// ```
    pub enum MembersView {
        /// The plume view (Free): temperature, precipitation and snow only.
        Plume => "plume",
        /// Every variable (Pro).
        Full => "full",
    }
}

string_enum! {
    /// Where current conditions came from.
    ///
    /// ```
    /// use ensemble_weather::CurrentSource;
    /// assert_eq!(CurrentSource::from("nowcast"), CurrentSource::Nowcast);
    /// ```
    pub enum CurrentSource {
        /// The observation-blended nowcast.
        Nowcast => "nowcast",
        /// The model forecast.
        Model => "model",
    }
}

string_enum! {
    /// Where a point forecast's elevation came from.
    ///
    /// ```
    /// use ensemble_weather::ElevationSource;
    /// assert_eq!(ElevationSource::from("request"), ElevationSource::Request);
    /// ```
    pub enum ElevationSource {
        /// The request's `elevation_m`.
        Request => "request",
        /// The 10 m 3DEP terrain model.
        Dem3dep10m => "dem_3dep_10m",
    }
}

string_enum! {
    /// The kind of a data source.
    ///
    /// ```
    /// use ensemble_weather::SourceKind;
    /// assert_eq!(SourceKind::from("model"), SourceKind::Model);
    /// ```
    pub enum SourceKind {
        /// A weather model.
        Model => "model",
        /// A peer forecast.
        Peer => "peer",
        /// Observations.
        Observation => "observation",
        /// Weather alerts.
        Alerts => "alerts",
        /// Climatology.
        Climatology => "climatology",
        /// Terrain.
        Terrain => "terrain",
        /// Place names.
        Places => "places",
        /// A base map.
        Basemap => "basemap",
    }
}

string_enum! {
    /// The kind of a temperature inversion.
    ///
    /// ```
    /// use ensemble_weather::InversionKind;
    /// assert_eq!(InversionKind::from("cold_pool"), InversionKind::ColdPool);
    /// ```
    pub enum InversionKind {
        /// A shallow surface inversion.
        SurfaceShallow => "surface_shallow",
        /// A cold pool.
        ColdPool => "cold_pool",
        /// An elevated inversion.
        Elevated => "elevated",
    }
}

string_enum! {
    /// The kind of a freezing level, which says which of its fields are set.
    ///
    /// ```
    /// use ensemble_weather::FreezingLevelKind;
    /// assert_eq!(FreezingLevelKind::from("height"), FreezingLevelKind::Height);
    /// ```
    pub enum FreezingLevelKind {
        /// The whole column is below freezing: no height.
        BelowSurface => "below_surface",
        /// A freezing height (`height`, `above_freezing_layer_aloft`).
        Height => "height",
        /// The whole profile is above freezing (`top`).
        AboveProfile => "above_profile",
    }
}

string_enum! {
    /// A moon phase.
    ///
    /// ```
    /// use ensemble_weather::MoonPhase;
    /// assert_eq!(MoonPhase::from("full_moon"), MoonPhase::FullMoon);
    /// ```
    pub enum MoonPhase {
        /// New moon.
        NewMoon => "new_moon",
        /// Waxing crescent.
        WaxingCrescent => "waxing_crescent",
        /// First quarter.
        FirstQuarter => "first_quarter",
        /// Waxing gibbous.
        WaxingGibbous => "waxing_gibbous",
        /// Full moon.
        FullMoon => "full_moon",
        /// Waning gibbous.
        WaningGibbous => "waning_gibbous",
        /// Last quarter.
        LastQuarter => "last_quarter",
        /// Waning crescent.
        WaningCrescent => "waning_crescent",
    }
}

string_enum! {
    /// An astronomical season.
    ///
    /// ```
    /// use ensemble_weather::Season;
    /// assert_eq!(Season::from("winter"), Season::Winter);
    /// ```
    pub enum Season {
        /// Spring.
        Spring => "spring",
        /// Summer.
        Summer => "summer",
        /// Autumn.
        Autumn => "autumn",
        /// Winter.
        Winter => "winter",
    }
}

string_enum! {
    /// A day's wind category.
    ///
    /// ```
    /// use ensemble_weather::WindStatus;
    /// assert_eq!(WindStatus::from("strong"), WindStatus::Strong);
    /// ```
    pub enum WindStatus {
        /// Calm.
        Calm => "calm",
        /// Light.
        Light => "light",
        /// Moderate.
        Moderate => "moderate",
        /// Strong.
        Strong => "strong",
        /// Very strong.
        VeryStrong => "very_strong",
        /// Extreme.
        Extreme => "extreme",
    }
}

string_enum! {
    /// The Columbia Gorge gap-flow category.
    ///
    /// ```
    /// use ensemble_weather::GapFlowStatus;
    /// assert_eq!(GapFlowStatus::from("none"), GapFlowStatus::None);
    /// ```
    pub enum GapFlowStatus {
        /// No gap flow.
        None => "none",
        /// Moderate.
        Moderate => "moderate",
        /// Strong.
        Strong => "strong",
        /// Extreme.
        Extreme => "extreme",
    }
}

string_enum! {
    /// How a storm's snow is layered.
    ///
    /// ```
    /// use ensemble_weather::StormOrientation;
    /// assert_eq!(StormOrientation::from("mixed"), StormOrientation::Mixed);
    /// ```
    pub enum StormOrientation {
        /// Denser snow under lighter snow.
        RightSideUp => "right_side_up",
        /// Lighter snow under denser snow.
        UpsideDown => "upside_down",
        /// Mixed.
        Mixed => "mixed",
    }
}

string_enum! {
    /// The kind of a catalog page-template section.
    ///
    /// ```
    /// use ensemble_weather::SectionKind;
    /// assert_eq!(SectionKind::from("media"), SectionKind::Media);
    /// ```
    pub enum SectionKind {
        /// A weather panel.
        WeatherPanel => "weather_panel",
        /// A content slot.
        ContentSlot => "content_slot",
        /// An ad slot.
        AdSlot => "ad_slot",
        /// Media.
        Media => "media",
    }
}

string_enum! {
    /// The kind of a catalog ad slot.
    ///
    /// ```
    /// use ensemble_weather::AdSlotKind;
    /// assert_eq!(AdSlotKind::from("house"), AdSlotKind::House);
    /// ```
    pub enum AdSlotKind {
        /// A house ad.
        House => "house",
        /// A third-party ad unit.
        ThirdParty => "third_party",
    }
}

string_enum! {
    /// A third-party ad provider.
    ///
    /// ```
    /// use ensemble_weather::AdProvider;
    /// assert_eq!(AdProvider::from("gam"), AdProvider::Gam);
    /// ```
    pub enum AdProvider {
        /// Google AdSense.
        AdSense => "adsense",
        /// Google Ad Manager.
        Gam => "gam",
        /// A custom provider.
        Custom => "custom",
    }
}

string_enum! {
    /// An option the server reduced to the caller's tier.
    ///
    /// ```
    /// use ensemble_weather::ClampedOption;
    /// assert_eq!(ClampedOption::from("max_hours"), ClampedOption::MaxHours);
    /// ```
    pub enum ClampedOption {
        /// `max_hours`.
        MaxHours => "max_hours",
        /// `max_days`.
        MaxDays => "max_days",
    }
}
