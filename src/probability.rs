//! Probability terms (`prob=`) and window totals (`totals=`).
//!
//! The server computes every probability and window total from the full
//! ensemble. This module only builds the canonical query (BEHAVIOR 4 and 5)
//! and maps the answers back to the caller's order; it never derives a
//! probability or sums quantiles.

use std::collections::HashMap;
use std::fmt;

use crate::enums::string_enum;
use crate::error::Error;
use crate::models::{ProbabilityResult, WindowTotal};
use crate::units::{Threshold, Units, convert};

string_enum! {
    /// A forecast variable a [`Chance`] or [`Total`] can ask about.
    ///
    /// ```
    /// use ensemble_weather::Variable;
    /// assert_eq!(Variable::Snowfall.as_str(), "snowfall");
    /// ```
    pub enum Variable {
        /// Air temperature.
        Temperature => "temperature",
        /// Wind speed.
        WindSpeed => "wind_speed",
        /// Snow level (the height where snow turns to rain).
        SnowLevel => "snow_level",
        /// Precipitation (liquid equivalent).
        Precipitation => "precipitation",
        /// Snowfall.
        Snowfall => "snowfall",
    }
}

impl Variable {
    fn is_amount(&self) -> bool {
        matches!(self, Self::Precipitation | Self::Snowfall)
    }

    fn default_aggregation(&self) -> Aggregation {
        if self.is_amount() {
            Aggregation::Total
        } else {
            Aggregation::Any
        }
    }

    /// The number of decimals the server serves for this variable in `units`
    /// (BEHAVIOR 4).
    fn precision(&self, units: &Units) -> usize {
        match self {
            Self::Precipitation | Self::Snowfall if *units == Units::Imperial => 3,
            Self::Precipitation | Self::Snowfall => 2,
            Self::SnowLevel => 0,
            _ => 1,
        }
    }
}

string_enum! {
    /// A probability comparison.
    ///
    /// ```
    /// use ensemble_weather::Op;
    /// assert_eq!(Op::AtLeast.as_str(), "ge");
    /// ```
    pub enum Op {
        /// The chance the value is at least the threshold (`ge`).
        AtLeast => "ge",
        /// The chance the value is at most the threshold (`le`).
        AtMost => "le",
    }
}

string_enum! {
    /// How a probability term or a window total combines the hours of its
    /// window.
    ///
    /// In a [`Chance`], precipitation and snowfall take only
    /// [`Aggregation::Total`]; the other variables take [`Aggregation::Any`]
    /// (their default) or [`Aggregation::All`]. [`Aggregation::Min`] and
    /// [`Aggregation::Max`] appear only in a [`WindowTotal`] the server
    /// answered.
    ///
    /// ```
    /// use ensemble_weather::Aggregation;
    /// assert_eq!(Aggregation::All.as_str(), "all");
    /// ```
    pub enum Aggregation {
        /// The member's total over the window crosses the threshold.
        Total => "total",
        /// Any hour of the window crosses the threshold.
        Any => "any",
        /// Every hour of the window crosses the threshold.
        All => "all",
        /// Each member's lowest value over the window (window totals only).
        Min => "min",
        /// Each member's highest value over the window (window totals only).
        Max => "max",
    }
}

impl Aggregation {
    fn rank(&self) -> u8 {
        match self {
            Self::Total => 0,
            Self::Any => 1,
            Self::All => 2,
            Self::Min => 3,
            Self::Max => 4,
            Self::Unknown(_) => 5,
        }
    }
}

/// The longest window the API answers, in hours.
pub const MAX_WINDOW_HOURS: u32 = 360;

/// A span of forecast hours counted from the first served hour.
///
/// The default is the first 24 hours.
///
/// ```
/// use ensemble_weather::Window;
/// assert_eq!(Window::hours(24).to_string(), "24h");
/// assert_eq!(Window::between(12, 36).to_string(), "12-36h");
/// assert_eq!(Window::default(), Window::hours(24));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Window {
    start: u32,
    end: u32,
}

impl Window {
    /// The first `n` hours (hours 0 to `n`).
    #[must_use]
    pub fn hours(n: u32) -> Self {
        Self { start: 0, end: n }
    }

    /// Hours `start` to `end`.
    #[must_use]
    pub fn between(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// The first hour of the window.
    #[must_use]
    pub fn start(&self) -> u32 {
        self.start
    }

    /// The end hour of the window.
    #[must_use]
    pub fn end(&self) -> u32 {
        self.end
    }

    fn check(self, operation: &'static str, what: &str) -> Result<Self, Error> {
        if self.start < self.end && self.end <= MAX_WINDOW_HOURS {
            Ok(self)
        } else {
            Err(Error::usage(
                operation,
                format!(
                    "{what}: window {}-{}h must satisfy start < end <= {MAX_WINDOW_HOURS}",
                    self.start, self.end
                ),
            ))
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::hours(24)
    }
}

impl fmt::Display for Window {
    /// The API's syntax: `24h`, or `12-36h` when the window starts later.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == 0 {
            write!(f, "{}h", self.end)
        } else {
            write!(f, "{}-{}h", self.start, self.end)
        }
    }
}

/// One probability term: the chance that a variable is at least (or at most)
/// a threshold over a window.
///
/// The server computes it from every ensemble member and answers it in
/// [`Forecast::probabilities`](crate::Forecast::probabilities), in the order
/// you asked.
///
/// ```
/// use ensemble_weather::{Aggregation, Chance, Threshold, Variable, Window};
///
/// // At least 6 in of snow in the next 24 h.
/// let snow = Chance::at_least(Variable::Snowfall, Threshold::inches(6.0));
/// // At most 32 °F in every hour from hour 6 to hour 18.
/// let freeze = Chance::at_most(Variable::Temperature, Threshold::fahrenheit(32.0))
///     .window(Window::between(6, 18))
///     .aggregation(Aggregation::All);
/// # let _ = (snow, freeze);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Chance {
    variable: Variable,
    op: Op,
    threshold: Threshold,
    window: Window,
    aggregation: Option<Aggregation>,
}

impl Chance {
    /// A term with an explicit comparison.
    #[must_use]
    pub fn new(variable: Variable, op: Op, threshold: Threshold) -> Self {
        Self {
            variable,
            op,
            threshold,
            window: Window::default(),
            aggregation: None,
        }
    }

    /// The chance `variable` is at least `threshold` (`ge`).
    #[must_use]
    pub fn at_least(variable: Variable, threshold: Threshold) -> Self {
        Self::new(variable, Op::AtLeast, threshold)
    }

    /// The chance `variable` is at most `threshold` (`le`).
    #[must_use]
    pub fn at_most(variable: Variable, threshold: Threshold) -> Self {
        Self::new(variable, Op::AtMost, threshold)
    }

    /// Sets the window (default: the first 24 hours).
    #[must_use]
    pub fn window(mut self, window: Window) -> Self {
        self.window = window;
        self
    }

    /// Sets the aggregation (default: total for precipitation and snowfall,
    /// any for the other variables).
    #[must_use]
    pub fn aggregation(mut self, aggregation: Aggregation) -> Self {
        self.aggregation = Some(aggregation);
        self
    }
}

/// One window total: the distribution of each ensemble member's own total
/// precipitation or snowfall over a window.
///
/// Answered in [`Forecast::window_totals`](crate::Forecast::window_totals) in
/// the order you asked. This is the only storm total the SDK offers: it never
/// sums hourly values (summing hourly quantiles gives a wrong total).
///
/// ```
/// use ensemble_weather::{Total, Variable, Window};
/// let storm = Total::new(Variable::Snowfall).window(Window::hours(72));
/// # let _ = storm;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Total {
    variable: Variable,
    window: Window,
}

impl Total {
    /// The total of `variable` (precipitation or snowfall) over the first
    /// 24 hours.
    #[must_use]
    pub fn new(variable: Variable) -> Self {
        Self {
            variable,
            window: Window::default(),
        }
    }

    /// Sets the window (default: the first 24 hours).
    #[must_use]
    pub fn window(mut self, window: Window) -> Self {
        self.window = window;
        self
    }
}

/// Rounds `v` exactly as the server does: half away from zero on `v × 10^d` in
/// IEEE-754 doubles, with -0 folded to 0, printed with exactly `d` decimals.
pub(crate) fn format_threshold(v: f64, d: usize) -> String {
    let p = 10f64.powi(i32::try_from(d).unwrap_or(0));
    let mut r = (v * p).round() / p;
    if r == 0.0 {
        r = 0.0; // fold -0
    }
    format!("{r:.d$}")
}

/// A validated term in request units, ready for the wire.
#[derive(Debug, Clone)]
struct Term {
    variable: Variable,
    op: Op,
    value: f64,
    text: String,
    window: Window,
    aggregation: Aggregation,
}

impl Term {
    /// The term's canonical spelling: what the SDK sends and the server
    /// echoes in `ProbabilityResult::term`.
    fn wire(&self) -> String {
        let mut s = format!(
            "{}:{}:{}:{}",
            self.variable, self.op, self.text, self.window
        );
        if self.aggregation != self.variable.default_aggregation() {
            s.push(':');
            s.push_str(self.aggregation.as_str());
        }
        s
    }
}

fn prepare(operation: &'static str, chance: &Chance, units: &Units) -> Result<Term, Error> {
    let usage = |msg: String| Error::usage(operation, msg);
    if chance.variable.is_unknown() {
        return Err(usage(format!(
            "chance: unknown variable {:?}",
            chance.variable.as_str()
        )));
    }
    if chance.op.is_unknown() {
        return Err(usage(format!(
            "chance: unknown op {:?}",
            chance.op.as_str()
        )));
    }
    let window = chance.window.check(operation, "chance")?;
    let aggregation = chance
        .aggregation
        .clone()
        .unwrap_or_else(|| chance.variable.default_aggregation());
    match (&aggregation, chance.variable.is_amount()) {
        (Aggregation::Unknown(raw), _) => {
            return Err(usage(format!("chance: unknown aggregation {raw:?}")));
        }
        (Aggregation::Min | Aggregation::Max, _) => {
            return Err(usage(format!(
                "chance: {aggregation} is a window-total aggregation; use any or all"
            )));
        }
        (Aggregation::Total, false) => {
            return Err(usage(format!(
                "chance: {} takes any or all, not total",
                chance.variable
            )));
        }
        (Aggregation::Any | Aggregation::All, true) => {
            return Err(usage(format!(
                "chance: {} takes only the total aggregation, not {aggregation}",
                chance.variable
            )));
        }
        _ => {}
    }
    let mut value = chance.threshold.value();
    if !value.is_finite() {
        return Err(usage(
            "chance: the threshold must be a finite number".to_owned(),
        ));
    }
    if let Some(unit) = chance.threshold.unit() {
        let target = units.unit_for(&chance.variable).ok_or_else(|| {
            usage(format!(
                "chance: cannot convert a threshold to the {units} unit system"
            ))
        })?;
        value = convert(value, unit, target).ok_or_else(|| {
            usage(format!(
                "chance: a {} threshold cannot be given in {unit}",
                chance.variable
            ))
        })?;
    }
    let text = format_threshold(value, chance.variable.precision(units));
    Ok(Term {
        variable: chance.variable.clone(),
        op: chance.op.clone(),
        value: text.parse().unwrap_or(value),
        text,
        window,
        aggregation,
    })
}

/// The canonical `prob=` value plus each caller term's canonical spelling, in
/// the caller's order.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChanceQuery {
    pub(crate) value: Option<String>,
    keys: Vec<String>,
}

pub(crate) fn build_chances(
    operation: &'static str,
    chances: &[Chance],
    units: &Units,
) -> Result<ChanceQuery, Error> {
    if chances.is_empty() {
        return Ok(ChanceQuery::default());
    }
    let mut keys = Vec::with_capacity(chances.len());
    let mut unique: HashMap<String, Term> = HashMap::new();
    for chance in chances {
        let term = prepare(operation, chance, units)?;
        let key = term.wire();
        keys.push(key.clone());
        unique.entry(key).or_insert(term);
    }
    let mut terms: Vec<Term> = unique.into_values().collect();
    terms.sort_by(|a, b| {
        a.variable
            .as_str()
            .cmp(b.variable.as_str())
            .then_with(|| a.op.as_str().cmp(b.op.as_str()))
            .then_with(|| a.window.cmp(&b.window))
            .then_with(|| a.aggregation.rank().cmp(&b.aggregation.rank()))
            .then_with(|| a.value.total_cmp(&b.value))
    });
    let wire: Vec<String> = terms.iter().map(Term::wire).collect();
    Ok(ChanceQuery {
        value: Some(wire.join(",")),
        keys,
    })
}

/// Matches the server's answers to the caller's terms by their echoed
/// canonical `term` (never by position), in the caller's order, duplicates
/// sharing one answer, and labels each threshold with its unit.
///
/// A response without any answers stays empty: nothing is invented. A
/// response that answers some terms but not all is an error.
pub(crate) fn map_chances(
    operation: &'static str,
    query: &ChanceQuery,
    results: Vec<ProbabilityResult>,
    units: &Units,
) -> Result<Vec<ProbabilityResult>, Error> {
    let mut results = results;
    for r in &mut results {
        r.threshold_unit = units.unit_for(&r.variable);
    }
    if query.keys.is_empty() || results.is_empty() {
        return Ok(results);
    }
    let by_key: HashMap<String, ProbabilityResult> =
        results.into_iter().map(|r| (r.term.clone(), r)).collect();
    query
        .keys
        .iter()
        .map(|k| {
            by_key.get(k).cloned().ok_or_else(|| {
                Error::unanswered(
                    operation,
                    format!("the response has no answer for probability term {k}"),
                )
            })
        })
        .collect()
}

/// The canonical `totals=` value plus each caller total's key.
#[derive(Debug, Clone, Default)]
pub(crate) struct TotalsQuery {
    pub(crate) value: Option<String>,
    keys: Vec<String>,
}

fn total_key(variable: &Variable, window: Window) -> String {
    format!("{variable}:{window}")
}

pub(crate) fn build_totals(
    operation: &'static str,
    totals: &[Total],
) -> Result<TotalsQuery, Error> {
    if totals.is_empty() {
        return Ok(TotalsQuery::default());
    }
    let mut keys = Vec::with_capacity(totals.len());
    let mut unique: Vec<(Variable, Window)> = Vec::new();
    for total in totals {
        if !total.variable.is_amount() {
            return Err(Error::usage(
                operation,
                format!(
                    "total: the variable must be precipitation or snowfall, not {}",
                    total.variable
                ),
            ));
        }
        let window = total.window.check(operation, "total")?;
        keys.push(total_key(&total.variable, window));
        if !unique
            .iter()
            .any(|(v, w)| *v == total.variable && *w == window)
        {
            unique.push((total.variable.clone(), window));
        }
    }
    unique.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()).then_with(|| a.1.cmp(&b.1)));
    let wire: Vec<String> = unique.iter().map(|(v, w)| total_key(v, *w)).collect();
    Ok(TotalsQuery {
        value: Some(wire.join(",")),
        keys,
    })
}

/// Matches window totals to the caller's terms by their echoed `term`, like
/// [`map_chances`].
pub(crate) fn map_totals(
    operation: &'static str,
    query: &TotalsQuery,
    results: Vec<WindowTotal>,
    units: &Units,
) -> Result<Vec<WindowTotal>, Error> {
    let mut results = results;
    for r in &mut results {
        r.unit = units.unit_for(&r.variable);
    }
    if query.keys.is_empty() || results.is_empty() {
        return Ok(results);
    }
    let by_key: HashMap<String, WindowTotal> =
        results.into_iter().map(|r| (r.term.clone(), r)).collect();
    query
        .keys
        .iter()
        .map(|k| {
            by_key.get(k).cloned().ok_or_else(|| {
                Error::unanswered(
                    operation,
                    format!("the response has no answer for window total {k}"),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_half_away_from_zero_in_doubles() {
        assert_eq!(format_threshold(0.0625, 3), "0.063");
        assert_eq!(format_threshold(1.005, 2), "1.00");
        assert_eq!(format_threshold(-0.04, 1), "0.0");
        assert_eq!(format_threshold(-5.0, 1), "-5.0");
        assert_eq!(format_threshold(1524.4, 0), "1524");
        assert_eq!(format_threshold(2.5, 0), "3");
        assert_eq!(format_threshold(-2.5, 0), "-3");
    }

    #[test]
    fn refuses_invalid_terms_before_sending() {
        let units = Units::Metric;
        let bad = [
            Chance::at_least(Variable::Precipitation, Threshold::millimeters(10.0))
                .aggregation(Aggregation::Any),
            Chance::at_least(Variable::Temperature, Threshold::celsius(0.0))
                .aggregation(Aggregation::Total),
            Chance::at_least(Variable::Temperature, Threshold::inches(1.0)),
            Chance::at_least(Variable::Snowfall, Threshold::millimeters(1.0))
                .window(Window::hours(361)),
            Chance::at_least(Variable::Snowfall, Threshold::millimeters(1.0))
                .window(Window::between(5, 5)),
            Chance::at_least(Variable::Snowfall, Threshold::millimeters(f64::NAN)),
            Chance::at_least(Variable::from("hail"), Threshold::request_units(1.0)),
        ];
        for chance in bad {
            let err = build_chances("getLocationForecast", std::slice::from_ref(&chance), &units)
                .unwrap_err();
            assert_eq!(err.category(), crate::Category::Usage, "{chance:?}");
        }
        assert!(build_totals("getLocationForecast", &[Total::new(Variable::Temperature)]).is_err());
    }

    #[test]
    fn totals_are_deduplicated_and_sorted() {
        let q = build_totals(
            "getLocationForecast",
            &[
                Total::new(Variable::Snowfall).window(Window::hours(240)),
                Total::new(Variable::Precipitation).window(Window::hours(48)),
                Total::new(Variable::Snowfall).window(Window::hours(240)),
            ],
        )
        .unwrap();
        assert_eq!(q.value.as_deref(), Some("precipitation:48h,snowfall:240h"));
        assert_eq!(q.keys.len(), 3);
    }
}
