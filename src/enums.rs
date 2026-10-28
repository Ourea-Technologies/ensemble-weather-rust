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

string_enum! {
    /// An icon hint for a condition, such as `night_snow`; front ends map it
    /// to their own artwork.
    ///
    /// ```
    /// use ensemble_weather::ConditionIcon;
    /// assert_eq!(ConditionIcon::from("night_snow"), ConditionIcon::NightSnow);
    /// ```
    pub enum ConditionIcon {
        /// `sunny`.
        Sunny => "sunny",
        /// `mostly_sunny`.
        MostlySunny => "mostly_sunny",
        /// `partly_cloudy`.
        PartlyCloudy => "partly_cloudy",
        /// `mostly_cloudy`.
        MostlyCloudy => "mostly_cloudy",
        /// `cloudy`.
        Cloudy => "cloudy",
        /// `sun_chance_showers`.
        SunChanceShowers => "sun_chance_showers",
        /// `sun_showers_likely`.
        SunShowersLikely => "sun_showers_likely",
        /// `sun_showers`.
        SunShowers => "sun_showers",
        /// `chance_showers`.
        ChanceShowers => "chance_showers",
        /// `chance_rain`.
        ChanceRain => "chance_rain",
        /// `showers_likely`.
        ShowersLikely => "showers_likely",
        /// `rain_likely`.
        RainLikely => "rain_likely",
        /// `showers`.
        Showers => "showers",
        /// `light_rain`.
        LightRain => "light_rain",
        /// `rain`.
        Rain => "rain",
        /// `heavy_rain`.
        HeavyRain => "heavy_rain",
        /// `rain_snow_mix`.
        RainSnowMix => "rain_snow_mix",
        /// `snow`.
        Snow => "snow",
        /// `freezing_rain`.
        FreezingRain => "freezing_rain",
        /// `night_clear`.
        NightClear => "night_clear",
        /// `night_mostly_clear`.
        NightMostlyClear => "night_mostly_clear",
        /// `night_partly_cloudy`.
        NightPartlyCloudy => "night_partly_cloudy",
        /// `night_mostly_cloudy`.
        NightMostlyCloudy => "night_mostly_cloudy",
        /// `night_cloudy`.
        NightCloudy => "night_cloudy",
        /// `night_moon_chance_showers`.
        NightMoonChanceShowers => "night_moon_chance_showers",
        /// `night_moon_showers_likely`.
        NightMoonShowersLikely => "night_moon_showers_likely",
        /// `night_moon_showers`.
        NightMoonShowers => "night_moon_showers",
        /// `night_chance_showers`.
        NightChanceShowers => "night_chance_showers",
        /// `night_chance_rain`.
        NightChanceRain => "night_chance_rain",
        /// `night_showers_likely`.
        NightShowersLikely => "night_showers_likely",
        /// `night_rain_likely`.
        NightRainLikely => "night_rain_likely",
        /// `night_showers`.
        NightShowers => "night_showers",
        /// `night_light_rain`.
        NightLightRain => "night_light_rain",
        /// `night_rain`.
        NightRain => "night_rain",
        /// `night_heavy_rain`.
        NightHeavyRain => "night_heavy_rain",
        /// `night_rain_snow_mix`.
        NightRainSnowMix => "night_rain_snow_mix",
        /// `night_snow`.
        NightSnow => "night_snow",
        /// `night_freezing_rain`.
        NightFreezingRain => "night_freezing_rain",
    }
}

string_enum! {
    /// An alert's CAP severity.
    ///
    /// ```
    /// use ensemble_weather::AlertSeverity;
    /// assert_eq!(AlertSeverity::from("severe"), AlertSeverity::Severe);
    /// ```
    pub enum AlertSeverity {
        /// Extreme.
        Extreme => "extreme",
        /// Severe.
        Severe => "severe",
        /// Moderate.
        Moderate => "moderate",
        /// Minor.
        Minor => "minor",
        /// The source says it is unknown (`"unknown"`); not to be confused
        /// with [`AlertSeverity::Unknown`], a value this SDK does not know.
        UnknownSeverity => "unknown",
    }
}

string_enum! {
    /// An alert's CAP urgency.
    ///
    /// ```
    /// use ensemble_weather::AlertUrgency;
    /// assert_eq!(AlertUrgency::from("expected"), AlertUrgency::Expected);
    /// ```
    pub enum AlertUrgency {
        /// Immediate.
        Immediate => "immediate",
        /// Expected.
        Expected => "expected",
        /// Future.
        Future => "future",
        /// Past.
        Past => "past",
        /// The source says it is unknown (`"unknown"`); not to be confused
        /// with [`AlertUrgency::Unknown`], a value this SDK does not know.
        UnknownUrgency => "unknown",
    }
}

string_enum! {
    /// An alert's CAP certainty.
    ///
    /// ```
    /// use ensemble_weather::AlertCertainty;
    /// assert_eq!(AlertCertainty::from("likely"), AlertCertainty::Likely);
    /// ```
    pub enum AlertCertainty {
        /// Observed.
        Observed => "observed",
        /// Likely.
        Likely => "likely",
        /// Possible.
        Possible => "possible",
        /// Unlikely.
        Unlikely => "unlikely",
        /// The source says it is unknown (`"unknown"`); not to be confused
        /// with [`AlertCertainty::Unknown`], a value this SDK does not know.
        UnknownCertainty => "unknown",
    }
}

string_enum! {
    /// An observation station's network.
    ///
    /// ```
    /// use ensemble_weather::StationNetwork;
    /// assert_eq!(StationNetwork::from("snotel"), StationNetwork::Snotel);
    /// ```
    pub enum StationNetwork {
        /// `snotel`.
        Snotel => "snotel",
        /// `snolite`.
        Snolite => "snolite",
        /// `raws`.
        Raws => "raws",
        /// `asos`.
        Asos => "asos",
        /// `awos`.
        Awos => "awos",
        /// `metar`.
        Metar => "metar",
        /// `rwis_wsdot`.
        RwisWsdot => "rwis_wsdot",
        /// `rwis_odot`.
        RwisOdot => "rwis_odot",
        /// `rwis_caltrans`.
        RwisCaltrans => "rwis_caltrans",
        /// `rwis_idot`.
        RwisIdot => "rwis_idot",
        /// `rwis_ndot`.
        RwisNdot => "rwis_ndot",
        /// `rwis_udot`.
        RwisUdot => "rwis_udot",
        /// `rwis_cdot`.
        RwisCdot => "rwis_cdot",
        /// `rwis_mdt`.
        RwisMdt => "rwis_mdt",
        /// `rwis_wydot`.
        RwisWydot => "rwis_wydot",
        /// `rwis_other`.
        RwisOther => "rwis_other",
        /// `agrimet`.
        Agrimet => "agrimet",
        /// `agweathernet`.
        Agweathernet => "agweathernet",
        /// `cocorahs`.
        Cocorahs => "cocorahs",
        /// `coop`.
        Coop => "coop",
        /// `usgs`.
        Usgs => "usgs",
        /// `tempest`.
        Tempest => "tempest",
        /// `davis`.
        Davis => "davis",
        /// `wxflow`.
        Wxflow => "wxflow",
        /// `cdec`.
        Cdec => "cdec",
        /// `cimis`.
        Cimis => "cimis",
        /// `mesonet_oregon`.
        MesonetOregon => "mesonet_oregon",
        /// `mesonet_synoptic`.
        MesonetSynoptic => "mesonet_synoptic",
        /// `mesonet_other`.
        MesonetOther => "mesonet_other",
        /// `ndbc_buoy`.
        NdbcBuoy => "ndbc_buoy",
        /// `other`.
        Other => "other",
        /// `hads`.
        Hads => "hads",
        /// `madis`.
        Madis => "madis",
        /// `nwac`.
        Nwac => "nwac",
        /// `mt_mesonet`.
        MtMesonet => "mt_mesonet",
    }
}

string_enum! {
    /// An observation station's live-feed status.
    ///
    /// ```
    /// use ensemble_weather::StationStatus;
    /// assert_eq!(StationStatus::from("active"), StationStatus::Active);
    /// ```
    pub enum StationStatus {
        /// Active.
        Active => "active",
        /// Intermittent.
        Intermittent => "intermittent",
        /// Inactive.
        Inactive => "inactive",
        /// Decommissioned.
        Decommissioned => "decommissioned",
        /// The source says it is unknown (`"unknown"`); not to be confused
        /// with [`StationStatus::Unknown`], a value this SDK does not know.
        UnknownStatus => "unknown",
    }
}

string_enum! {
    /// A catalog page template variant.
    ///
    /// ```
    /// use ensemble_weather::TemplateVariant;
    /// assert_eq!(TemplateVariant::from("mountain"), TemplateVariant::Mountain);
    /// ```
    pub enum TemplateVariant {
        /// Mountain.
        Mountain => "mountain",
        /// Coastal.
        Coastal => "coastal",
        /// Trail.
        Trail => "trail",
        /// Pass.
        Pass => "pass",
        /// Generic.
        Generic => "generic",
        /// Station.
        Station => "station",
    }
}

string_enum! {
    /// Why the server could not answer a probability or a window total.
    ///
    /// ```
    /// use ensemble_weather::NullReason;
    /// assert_eq!(NullReason::from("window_not_covered"), NullReason::WindowNotCovered);
    /// ```
    pub enum NullReason {
        /// The served series does not reach the window's end.
        WindowNotCovered => "window_not_covered",
        /// Fewer than two members cover the whole window.
        TooFewMembers => "too_few_members",
        /// No member carries the variable through the window (for example
        /// with `hourly(false)`, which serves no members).
        VariableUnavailable => "variable_unavailable",
    }
}
