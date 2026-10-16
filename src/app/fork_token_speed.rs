//! [fork] token speed: output tokens per second for the selected session,
//! shown in the composer footer. Finished model calls contribute the exact
//! output tokens and generation time the driver measured (`ForkStepUsage`);
//! the call still streaming is estimated from its text so the live value moves
//! between them. The rate excludes waiting for the first token and tool
//! execution. Frames only read the stored rates.

use super::*;

/// How often the live estimate may change, so the number stays readable.
const LIVE_REFRESH: Duration = Duration::from_millis(500);
/// Windows shorter than this give unstable rates: they are neither shown nor
/// counted toward the turn's average.
const MIN_WINDOW_MS: u64 = 300;

/// Providers whose drivers report `ForkStepUsage`.
fn reports_step_usage(provider: ProviderKind) -> bool {
    matches!(
        provider,
        ProviderKind::Claude | ProviderKind::Codex | ProviderKind::Pi | ProviderKind::OhMyPi
    )
}

/// Streamed text carries no token counts: count CJK characters as one token
/// each and everything else at four characters per token.
fn estimate_tokens(text: &str) -> f64 {
    let (cjk, other) = text.chars().fold((0_u32, 0_u32), |(cjk, other), ch| {
        if matches!(ch, '\u{3000}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' | '\u{ff00}'..='\u{ffef}')
        {
            (cjk + 1, other)
        } else {
            (cjk, other + 1)
        }
    });
    f64::from(cjk) + f64::from(other) / 4.0
}

#[derive(Default)]
pub(super) struct TokenSpeed {
    turn_id: Option<Uuid>,
    /// Exact output tokens and generation time of the turn's finished calls.
    turn_output: u64,
    turn_ms: u64,
    /// The call in progress, on the client clock; only feeds the estimate.
    step_started: Option<Instant>,
    step_estimate: f64,
    shown_at: Option<Instant>,
    live: Option<f64>,
}

impl TokenSpeed {
    fn turn_rate(&self) -> Option<f64> {
        (self.turn_ms >= MIN_WINDOW_MS)
            .then(|| self.turn_output as f64 / (self.turn_ms as f64 / 1000.0))
    }

    fn delta(&mut self, text: &str) {
        let now = Instant::now();
        let started = *self.step_started.get_or_insert(now);
        self.step_estimate += estimate_tokens(text);
        if self
            .shown_at
            .is_some_and(|shown_at| now.duration_since(shown_at) < LIVE_REFRESH)
        {
            return;
        }
        let ms = self.turn_ms + now.duration_since(started).as_millis() as u64;
        if ms >= MIN_WINDOW_MS {
            self.live = Some((self.turn_output as f64 + self.step_estimate) / (ms as f64 / 1000.0));
            self.shown_at = Some(now);
        }
    }

    fn step(&mut self, output_tokens: u64, generation_ms: u64) {
        self.step_started = None;
        self.step_estimate = 0.0;
        if generation_ms >= MIN_WINDOW_MS {
            self.turn_output += output_tokens;
            self.turn_ms += generation_ms;
            self.live = self.turn_rate();
        }
    }
}

impl Waku {
    /// The speed record for `session_id`'s turn in flight, started fresh when
    /// a new turn began. `None` outside a turn.
    fn fork_token_speed_for_turn(&mut self, session_id: Uuid) -> Option<&mut TokenSpeed> {
        let turn_id = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)?
            .active_turn_id()?;
        let speed = self.fork_token_speeds.entry(session_id).or_default();
        if speed.turn_id != Some(turn_id) {
            *speed = TokenSpeed {
                turn_id: Some(turn_id),
                ..TokenSpeed::default()
            };
        }
        Some(speed)
    }

    pub(super) fn fork_token_speed_delta(&mut self, session_id: Uuid, text: &str) {
        if let Some(speed) = self.fork_token_speed_for_turn(session_id) {
            speed.delta(text);
        }
    }

    pub(super) fn fork_token_speed_step(
        &mut self,
        session_id: Uuid,
        output_tokens: u64,
        generation_ms: u64,
    ) {
        if let Some(speed) = self.fork_token_speed_for_turn(session_id) {
            speed.step(output_tokens, generation_ms);
        }
    }

    /// `Live 42 t/s` while the selected session's turn runs, `Turn 38 t/s`
    /// after it, and `--` whenever there is no rate (too short to measure, or
    /// nothing recorded since launch). Always shown for providers that report
    /// step usage, so the footer never shifts.
    pub(super) fn render_fork_token_speed(&self, cx: &App) -> Option<AnyElement> {
        let session = self.selected_session()?;
        if !reports_step_usage(session.provider) {
            return None;
        }
        let speed = self.fork_token_speeds.get(&session.id);
        let (label, rate) = match session.active_turn_id() {
            Some(turn_id) if session.status.is_busy() => (
                tr!("fork.token_speed.live"),
                speed
                    .filter(|speed| speed.turn_id == Some(turn_id))
                    .and_then(|speed| speed.live),
            ),
            _ => (
                tr!("fork.token_speed.turn"),
                speed.and_then(TokenSpeed::turn_rate),
            ),
        };
        let theme = Theme::current(cx);
        let rate = rate.map_or_else(
            || "--".to_owned(),
            |rate| format!("{}", rate.round() as u64),
        );
        Some(
            div()
                .flex_none()
                .px(px(6.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_color(theme.text_secondary)
                .child(label)
                .child(div().text_color(theme.text).child(rate))
                .child("t/s")
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_counts_cjk_as_whole_tokens() {
        assert_eq!(estimate_tokens("你好"), 2.0);
        assert_eq!(estimate_tokens("abcdefgh"), 2.0);
    }

    #[test]
    fn short_steps_do_not_count_toward_the_turn() {
        let mut speed = TokenSpeed::default();
        speed.step(1_000, MIN_WINDOW_MS - 1);
        assert_eq!(speed.turn_rate(), None);

        speed.step(100, 2_000);
        speed.step(50, 500);
        assert_eq!(speed.turn_rate(), Some(60.0));
        assert_eq!(speed.live, Some(60.0));
    }
}
