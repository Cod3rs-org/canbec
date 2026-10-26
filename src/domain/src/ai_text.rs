//! AI text hints (#25): when a detector score is worth showing. The model (Ettin-68M) flags 5% of
//! human texts at 50%, so the hint waits for a much higher score and stays a quiet chip, never a
//! blur or a block (docs/research/09-local-ai-text-hints.md).

/// Lowest P(AI-generated) that becomes a hint.
/// ponytail: picked by hand above the model's 5%-false-positive point; tune with real pages.
pub const HINT_AT: f64 = 0.85;

/// The hint as a percentage, or `None` below [`HINT_AT`] (and for nonsense scores).
pub fn hint(p: f64) -> Option<u8> {
    (HINT_AT..=1.0).contains(&p).then(|| (p * 100.0).round() as u8)
}
