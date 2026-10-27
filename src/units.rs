//! Unit systems, units and probability thresholds.

use crate::enums::string_enum;
use crate::probability::Variable;

string_enum! {
    /// A unit system for a request and its response.
    ///
    /// The server converts every value; the SDK never converts response
    /// values (BEHAVIOR 7). The default is [`Units::Metric`].
    ///
    /// | Units | Temperature | Precipitation, snowfall | Wind | Heights |
    /// |---|---|---|---|---|
    /// | `Metric` | °C | mm | m/s | m |
    /// | `Imperial` | °F | in | mph | ft |
    /// | `Kelvin` | K | mm | m/s | m |
    ///
    /// ```
    /// use ensemble_weather::{Unit, Units, Variable};
    /// assert_eq!(Units::Imperial.unit_for(&Variable::Snowfall), Some(Unit::Inch));
    /// ```
    pub enum Units {
        /// °C, mm, m/s, m. The API default.
        Metric => "metric",
        /// °F, in, mph, ft.
        Imperial => "imperial",
        /// Metric, with temperatures in K.
        Kelvin => "kelvin",
    }
}

#[allow(clippy::derivable_impls)] // the enum is macro-generated
impl Default for Units {
    fn default() -> Self {
        Self::Metric
    }
}

impl Units {
    /// The unit the server uses for `variable` in this unit system, or
    /// `None` for an unknown unit system or variable.
    #[must_use]
    pub fn unit_for(&self, variable: &Variable) -> Option<Unit> {
        let imperial = match self {
            Self::Metric | Self::Kelvin => false,
            Self::Imperial => true,
            Self::Unknown(_) => return None,
        };
        Some(match variable {
            Variable::Temperature => match self {
                Self::Imperial => Unit::Fahrenheit,
                Self::Kelvin => Unit::Kelvin,
                _ => Unit::Celsius,
            },
            Variable::Precipitation | Variable::Snowfall if imperial => Unit::Inch,
            Variable::Precipitation | Variable::Snowfall => Unit::Millimeter,
            Variable::SnowLevel if imperial => Unit::Foot,
            Variable::SnowLevel => Unit::Meter,
            Variable::WindSpeed if imperial => Unit::MilePerHour,
            Variable::WindSpeed => Unit::MeterPerSecond,
            Variable::Unknown(_) => return None,
        })
    }
}

/// The unit of a single quantity, such as a probability threshold.
///
/// ```
/// use ensemble_weather::Unit;
/// assert_eq!(Unit::Celsius.symbol(), "°C");
/// assert_eq!(Unit::Inch.symbol(), "in");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Unit {
    /// Millimetres.
    Millimeter,
    /// Inches.
    Inch,
    /// Degrees Celsius.
    Celsius,
    /// Degrees Fahrenheit.
    Fahrenheit,
    /// Kelvin.
    Kelvin,
    /// Metres.
    Meter,
    /// Feet.
    Foot,
    /// Metres per second.
    MeterPerSecond,
    /// Miles per hour.
    MilePerHour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Depth,
    Temperature,
    Height,
    Speed,
}

/// Exact conversion factors (BEHAVIOR 7). They convert the caller's threshold
/// input only; response values are never converted.
const MM_PER_INCH: f64 = 25.4;
const METERS_PER_FOOT: f64 = 0.3048;
const MPS_PER_MPH: f64 = 0.44704;
const KELVIN_OFFSET: f64 = 273.15;

impl Unit {
    /// The unit's display symbol, such as `"°C"` or `"m/s"`.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Millimeter => "mm",
            Self::Inch => "in",
            Self::Celsius => "°C",
            Self::Fahrenheit => "°F",
            Self::Kelvin => "K",
            Self::Meter => "m",
            Self::Foot => "ft",
            Self::MeterPerSecond => "m/s",
            Self::MilePerHour => "mph",
        }
    }

    fn dimension(self) -> Dimension {
        match self {
            Self::Millimeter | Self::Inch => Dimension::Depth,
            Self::Celsius | Self::Fahrenheit | Self::Kelvin => Dimension::Temperature,
            Self::Meter | Self::Foot => Dimension::Height,
            Self::MeterPerSecond | Self::MilePerHour => Dimension::Speed,
        }
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.symbol())
    }
}

/// Converts `v` from `from` to `to`, or `None` when they measure different
/// things. A value already in the target unit is returned unchanged.
pub(crate) fn convert(v: f64, from: Unit, to: Unit) -> Option<f64> {
    if from.dimension() != to.dimension() {
        return None;
    }
    if from == to {
        return Some(v);
    }
    Some(match (from, to) {
        (Unit::Inch, _) => v * MM_PER_INCH,
        (Unit::Millimeter, _) => v / MM_PER_INCH,
        (Unit::Foot, _) => v * METERS_PER_FOOT,
        (Unit::Meter, _) => v / METERS_PER_FOOT,
        (Unit::MilePerHour, _) => v * MPS_PER_MPH,
        (Unit::MeterPerSecond, _) => v / MPS_PER_MPH,
        _ => {
            let celsius = match from {
                Unit::Fahrenheit => (v - 32.0) * 5.0 / 9.0,
                Unit::Kelvin => v - KELVIN_OFFSET,
                _ => v,
            };
            match to {
                Unit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
                Unit::Kelvin => celsius + KELVIN_OFFSET,
                _ => celsius,
            }
        }
    })
}

/// A probability threshold: a value, optionally with its unit.
///
/// A threshold with a unit is converted to the request's units with exact
/// factors before it is sent (6 in on a metric request is sent as 152.40 mm).
/// This converts your input only; it is not statistics. A threshold made with
/// [`Threshold::request_units`] is sent as it is.
///
/// ```
/// use ensemble_weather::{Threshold, Unit};
/// let six_inches = Threshold::inches(6.0);
/// assert_eq!(six_inches.value(), 6.0);
/// assert_eq!(six_inches.unit(), Some(Unit::Inch));
/// assert_eq!(Threshold::request_units(15.0).unit(), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threshold {
    value: f64,
    unit: Option<Unit>,
}

impl Threshold {
    /// A threshold of `value` in `unit`.
    #[must_use]
    pub fn new(value: f64, unit: Unit) -> Self {
        Self {
            value,
            unit: Some(unit),
        }
    }

    /// A threshold already in the request's units, sent as it is (after the
    /// server's rounding).
    #[must_use]
    pub fn request_units(value: f64) -> Self {
        Self { value, unit: None }
    }

    /// `value` mm.
    #[must_use]
    pub fn millimeters(value: f64) -> Self {
        Self::new(value, Unit::Millimeter)
    }

    /// `value` in.
    #[must_use]
    pub fn inches(value: f64) -> Self {
        Self::new(value, Unit::Inch)
    }

    /// `value` °C.
    #[must_use]
    pub fn celsius(value: f64) -> Self {
        Self::new(value, Unit::Celsius)
    }

    /// `value` °F.
    #[must_use]
    pub fn fahrenheit(value: f64) -> Self {
        Self::new(value, Unit::Fahrenheit)
    }

    /// `value` K.
    #[must_use]
    pub fn kelvin(value: f64) -> Self {
        Self::new(value, Unit::Kelvin)
    }

    /// `value` m.
    #[must_use]
    pub fn meters(value: f64) -> Self {
        Self::new(value, Unit::Meter)
    }

    /// `value` ft.
    #[must_use]
    pub fn feet(value: f64) -> Self {
        Self::new(value, Unit::Foot)
    }

    /// `value` m/s.
    #[must_use]
    pub fn meters_per_second(value: f64) -> Self {
        Self::new(value, Unit::MeterPerSecond)
    }

    /// `value` mph.
    #[must_use]
    pub fn miles_per_hour(value: f64) -> Self {
        Self::new(value, Unit::MilePerHour)
    }

    /// The value, in [`Threshold::unit`].
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// The unit, or `None` for a value already in the request's units.
    #[must_use]
    pub fn unit(&self) -> Option<Unit> {
        self.unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_use_exact_factors() {
        assert_eq!(
            convert(6.0, Unit::Inch, Unit::Millimeter),
            Some(152.399_999_999_999_98)
        );
        assert_eq!(convert(32.0, Unit::Fahrenheit, Unit::Celsius), Some(0.0));
        assert_eq!(convert(0.0, Unit::Celsius, Unit::Kelvin), Some(273.15));
        assert_eq!(convert(1.0, Unit::Foot, Unit::Meter), Some(0.3048));
        assert_eq!(
            convert(1.0, Unit::MilePerHour, Unit::MeterPerSecond),
            Some(0.44704)
        );
        assert_eq!(convert(1.0, Unit::Inch, Unit::Celsius), None);
        assert_eq!(convert(5.0, Unit::Meter, Unit::Meter), Some(5.0));
    }
}
