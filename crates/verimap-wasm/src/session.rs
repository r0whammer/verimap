use verimap_domain::{
    ApplyError, Changes, Command, CommandError, DomainError, EVENT_SCHEMA_VERSION,
    EXPIRE_AFTER_TICKS, ObjectId, STALE_AFTER_TICKS, ScenePoint, SceneStore, Slot, TICK_MS,
};
use verimap_protocol::bridge::{
    CoreEvent, ErrorCode, PlacedPointDto, RequestError, UiRequest, parse_request,
};
use verimap_protocol::scene::{
    AnnotationDto, EntityDetailDto, EntityDto, FixDto, IngestStatsDto, ProblemDto, SceneUpdateDto,
    SessionDto, TimelineDto, ViewModeDto,
};
use verimap_protocol::site::SiteDto;
use verimap_protocol::{PROTOCOL_VERSION, ProtocolVersion, U64String};
use verimap_sim::{DEMO_SEED, LocalSession, SCENE_VERSION, SIMULATOR_VERSION};

/// Build identifier baked in at compile time from `VERIMAP_BUILD_ID`
/// (set by the justfile from git), or `dev`.
pub const CORE_BUILD: &str = match option_env!("VERIMAP_BUILD_ID") {
    Some(id) => id,
    None => "dev",
};

/// Sent if an event cannot be serialized, which would be a core bug.
const SERIALIZE_FAILED: &str =
    r#"{"type":"error","code":"internal","message":"failed to serialize core event"}"#;

/// A past state rebuilt from the journal. The live session is untouched.
#[derive(Debug, Clone)]
struct Review {
    store: SceneStore,
}

/// Core state for one worker: the negotiated protocol, the live local
/// session, and an optional review of its history. Platform independent, so
/// native tests cover exactly what the WASM build runs.
#[derive(Debug, Default)]
pub struct CoreSession {
    negotiated: Option<ProtocolVersion>,
    live: Option<LocalSession>,
    review: Option<Review>,
}

impl CoreSession {
    /// A session awaiting `hello`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The protocol agreed with the UI, if the handshake succeeded.
    pub fn negotiated(&self) -> Option<ProtocolVersion> {
        self.negotiated
    }

    /// Handles one JSON request and returns one JSON event. Never panics on
    /// malformed input; rejected requests leave the session unchanged.
    pub fn handle_json(&mut self, request_json: &str) -> String {
        let event = match parse_request(request_json) {
            Ok(request) => self.handle(request),
            Err(e) => e.into(),
        };
        serde_json::to_string(&event).unwrap_or_else(|_| SERIALIZE_FAILED.to_owned())
    }

    /// Handles one validated request.
    pub fn handle(&mut self, request: UiRequest) -> CoreEvent {
        if let UiRequest::Hello { protocol, .. } = request {
            return self.hello(protocol);
        }
        let (Some(_), Some(live)) = (self.negotiated, self.live.as_mut()) else {
            return error(ErrorCode::HandshakeRequired, "send hello first");
        };
        match handle_negotiated(live, &mut self.review, request) {
            Ok(event) => event,
            Err(e) => e.into(),
        }
    }

    fn hello(&mut self, ui_protocol: ProtocolVersion) -> CoreEvent {
        if !PROTOCOL_VERSION.serves(ui_protocol) {
            self.negotiated = None;
            return CoreEvent::Incompatible {
                core_protocol: PROTOCOL_VERSION,
                ui_protocol,
            };
        }
        if self.live.is_none() {
            match LocalSession::new(DEMO_SEED) {
                Ok(s) => self.live = Some(s),
                Err(e) => {
                    return error(
                        ErrorCode::Internal,
                        format!("bundled demo session is invalid: {e}"),
                    );
                }
            }
        }
        let Some(live) = self.live.as_ref() else {
            return error(ErrorCode::Internal, "session missing after start");
        };
        self.negotiated = Some(ui_protocol);
        // A repeated hello returns to the live view.
        self.review = None;
        let store = live.authority().store();
        CoreEvent::Ready {
            protocol: PROTOCOL_VERSION,
            core_build: CORE_BUILD.to_owned(),
            site: SiteDto::from(store.site()),
            session: SessionDto {
                seed: live.seed().into(),
                simulator_version: SIMULATOR_VERSION,
                scene_version: SCENE_VERSION,
                event_schema_version: EVENT_SCHEMA_VERSION,
                tick_ms: u32::try_from(TICK_MS).unwrap_or(u32::MAX),
                stale_after_ticks: u32::try_from(STALE_AFTER_TICKS).unwrap_or(u32::MAX),
                expire_after_ticks: u32::try_from(EXPIRE_AFTER_TICKS).unwrap_or(u32::MAX),
            },
            scene: full_update(live, None),
        }
    }
}

fn error(code: ErrorCode, message: impl Into<String>) -> CoreEvent {
    CoreEvent::Error {
        code,
        message: message.into(),
    }
}

fn handle_negotiated(
    live: &mut LocalSession,
    review: &mut Option<Review>,
    request: UiRequest,
) -> Result<CoreEvent, RequestError> {
    let scene = |update| CoreEvent::Scene { update };
    match request {
        // Handled before negotiation.
        UiRequest::Hello { .. } => Err(RequestError::new(ErrorCode::Internal, "unexpected hello")),
        UiRequest::Advance { ticks } => {
            require_live(review)?;
            let advanced = live.advance(ticks);
            let problem = advanced.stopped.as_ref().map(problem);
            let mut update = delta_update(live, None, &advanced.changes);
            update.problem = problem;
            Ok(scene(update))
        }
        UiRequest::AddMarker { label, at } => {
            require_live(review)?;
            let (position, placement) = placed(&at)?;
            let changes = live
                .submit(Command::AddMarker {
                    label,
                    position,
                    placement,
                })
                .map_err(command_error)?;
            Ok(scene(delta_update(live, None, &changes)))
        }
        UiRequest::EditMarker {
            id,
            base_revision,
            label,
            at,
        } => {
            require_live(review)?;
            let position = at.as_ref().map(placed).transpose()?;
            let changes = live
                .submit(Command::EditMarker {
                    id: object_id(&id)?,
                    base_revision: base_revision.0,
                    label,
                    position,
                })
                .map_err(command_error)?;
            Ok(scene(delta_update(live, None, &changes)))
        }
        UiRequest::DeleteAnnotation { id, base_revision } => {
            require_live(review)?;
            let changes = live
                .submit(Command::DeleteAnnotation {
                    id: object_id(&id)?,
                    base_revision: base_revision.0,
                })
                .map_err(command_error)?;
            Ok(scene(delta_update(live, None, &changes)))
        }
        UiRequest::RemoveEntity { id, base_revision } => {
            require_live(review)?;
            let changes = live
                .submit(Command::RemoveEntity {
                    id: object_id(&id)?,
                    base_revision: base_revision.0,
                })
                .map_err(command_error)?;
            Ok(scene(delta_update(live, None, &changes)))
        }
        UiRequest::InspectEntity { id } => {
            let id = object_id(&id)?;
            let store = view_store(live, review.as_ref());
            let Some((revision, entity)) = store.entity(&id) else {
                let code = match store.entity_slot(&id) {
                    Slot::Deleted { .. } => ErrorCode::Deleted,
                    _ => ErrorCode::NotFound,
                };
                return Err(RequestError::new(code, format!("{id} is not in this view")));
            };
            Ok(CoreEvent::EntityDetail {
                detail: EntityDetailDto {
                    entity: EntityDto::new(entity, revision, store.tick()),
                    history: entity.history().map(FixDto::from).collect(),
                },
            })
        }
        UiRequest::Review { tick } => {
            let authority = live.authority();
            let tick = tick.0.min(authority.store().tick());
            // The journal is in tick order: include every event at or
            // before `tick`.
            let n = authority.journal().partition_point(|e| e.tick <= tick);
            let sequence = u64::try_from(n).unwrap_or(u64::MAX);
            let store = authority.replay_to(sequence).map_err(replay_error)?;
            let r = review.insert(Review { store });
            Ok(scene(full_update(live, Some(r))))
        }
        UiRequest::ReviewStep { ticks } => {
            let Some(r) = review.as_mut() else {
                return Err(RequestError::new(ErrorCode::WrongMode, "not reviewing"));
            };
            let journal = live.authority().journal();
            let target = r.store.tick().saturating_add(u64::from(ticks));
            let from = usize::try_from(r.store.last_sequence()).unwrap_or(usize::MAX);
            let mut changes = Changes::default();
            for event in journal.get(from..).unwrap_or(&[]) {
                if event.tick > target {
                    break;
                }
                changes.merge(r.store.apply(event).map_err(replay_error)?);
            }
            Ok(scene(delta_update(live, Some(r), &changes)))
        }
        UiRequest::GoLive {} => {
            *review = None;
            Ok(scene(full_update(live, None)))
        }
        UiRequest::StateHash {} => {
            let store = view_store(live, review.as_ref());
            Ok(CoreEvent::StateHash {
                tick: store.tick().into(),
                sequence: store.last_sequence().into(),
                hash: format!("{:016x}", store.state_hash()),
            })
        }
    }
}

fn require_live(review: &Option<Review>) -> Result<(), RequestError> {
    if review.is_some() {
        return Err(RequestError::new(
            ErrorCode::WrongMode,
            "reviewing history is read-only; return to live first",
        ));
    }
    Ok(())
}

fn object_id(id: &str) -> Result<ObjectId, RequestError> {
    ObjectId::parse("id", id).map_err(invalid)
}

fn placed(at: &PlacedPointDto) -> Result<(ScenePoint, verimap_domain::Placement), RequestError> {
    let p = at.position;
    let position = ScenePoint::from_metres(p.x_m, p.y_m, p.z_m).map_err(invalid)?;
    Ok((position, at.placement.into()))
}

fn invalid(e: DomainError) -> RequestError {
    RequestError::new(ErrorCode::InvalidField, e.to_string())
}

fn command_code(e: &CommandError) -> ErrorCode {
    match e {
        CommandError::Invalid(_) => ErrorCode::InvalidField,
        CommandError::NotFound { .. } => ErrorCode::NotFound,
        CommandError::Deleted { .. } => ErrorCode::Deleted,
        CommandError::PreconditionFailed { .. } => ErrorCode::PreconditionFailed,
        CommandError::JournalFull { .. } | CommandError::SequenceExhausted => {
            ErrorCode::JournalFull
        }
        CommandError::NothingToChange
        | CommandError::IdSpaceExhausted
        | CommandError::Rejected(_) => ErrorCode::Rejected,
    }
}

fn command_error(e: CommandError) -> RequestError {
    RequestError::new(command_code(&e), e.to_string())
}

fn problem(e: &CommandError) -> ProblemDto {
    ProblemDto {
        code: command_code(e),
        message: e.to_string(),
    }
}

/// A committed journal that does not replay is a core bug.
fn replay_error(e: ApplyError) -> RequestError {
    RequestError::new(ErrorCode::Internal, format!("journal replay failed: {e}"))
}

fn view_store<'a>(live: &'a LocalSession, review: Option<&'a Review>) -> &'a SceneStore {
    review.map_or_else(|| live.authority().store(), |r| &r.store)
}

fn timeline(live: &LocalSession, review: Option<&Review>) -> TimelineDto {
    let authority = live.authority();
    let view = view_store(live, review);
    let n = |v: usize| U64String(u64::try_from(v).unwrap_or(u64::MAX));
    TimelineDto {
        mode: if review.is_some() {
            ViewModeDto::Review
        } else {
            ViewModeDto::Live
        },
        tick: view.tick().into(),
        sequence: view.last_sequence().into(),
        live_tick: authority.store().tick().into(),
        live_sequence: authority.store().last_sequence().into(),
        journal_events: n(authority.journal().len()),
        journal_limit: n(authority.journal_limit()),
    }
}

fn stats(live: &LocalSession) -> IngestStatsDto {
    let s = live.stats();
    IngestStatsDto {
        received: s.received.into(),
        accepted: s.accepted.into(),
        rejected: s.rejected.into(),
        duplicates: s.duplicates.into(),
        late: s.late.into(),
    }
}

/// The complete view.
fn full_update(live: &LocalSession, review: Option<&Review>) -> SceneUpdateDto {
    let store = view_store(live, review);
    SceneUpdateDto {
        replace: true,
        timeline: timeline(live, review),
        stats: stats(live),
        entities: store
            .entities()
            .map(|(r, e)| EntityDto::new(e, r, store.tick()))
            .collect(),
        removed_entities: Vec::new(),
        annotations: store
            .annotations()
            .map(|(r, a)| AnnotationDto::new(a, r))
            .collect(),
        removed_annotations: Vec::new(),
        problem: None,
    }
}

/// The objects in `changes`, as they are now in the view.
fn delta_update(live: &LocalSession, review: Option<&Review>, changes: &Changes) -> SceneUpdateDto {
    let store = view_store(live, review);
    let mut update = SceneUpdateDto {
        replace: false,
        timeline: timeline(live, review),
        stats: stats(live),
        entities: Vec::new(),
        removed_entities: Vec::new(),
        annotations: Vec::new(),
        removed_annotations: Vec::new(),
        problem: None,
    };
    for id in &changes.entities {
        match store.entity(id) {
            Some((r, e)) => update.entities.push(EntityDto::new(e, r, store.tick())),
            None => update.removed_entities.push(id.as_str().to_owned()),
        }
    }
    for id in &changes.annotations {
        match store.annotation(id) {
            Some((r, a)) => update.annotations.push(AnnotationDto::new(a, r)),
            None => update.removed_annotations.push(id.as_str().to_owned()),
        }
    }
    update
}

#[cfg(test)]
mod tests {
    use super::*;
    use verimap_domain::SiteFrame;
    use verimap_protocol::scene::FreshnessDto;

    fn hello(major: u16, minor: u16) -> String {
        format!(
            r#"{{"type":"hello","protocol":{{"major":{major},"minor":{minor}}},"ui_build":"test"}}"#
        )
    }

    fn event(json: &str) -> CoreEvent {
        serde_json::from_str(json).expect("core always emits a valid CoreEvent")
    }

    fn ready() -> CoreSession {
        let mut s = CoreSession::new();
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor));
        assert!(matches!(event(&out), CoreEvent::Ready { .. }), "{out}");
        s
    }

    fn send(s: &mut CoreSession, json: &str) -> CoreEvent {
        event(&s.handle_json(json))
    }

    fn update(e: CoreEvent) -> SceneUpdateDto {
        match e {
            CoreEvent::Scene { update } => update,
            other => panic!("expected a scene update, got {other:?}"),
        }
    }

    fn error_code(e: CoreEvent) -> ErrorCode {
        match e {
            CoreEvent::Error { code, .. } => code,
            other => panic!("expected an error, got {other:?}"),
        }
    }

    fn state_hash(s: &mut CoreSession) -> String {
        match send(s, r#"{"type":"state_hash"}"#) {
            CoreEvent::StateHash { hash, .. } => hash,
            other => panic!("expected a hash, got {other:?}"),
        }
    }

    #[test]
    fn matching_hello_is_ready_with_valid_site_and_empty_scene() {
        let mut s = CoreSession::new();
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor));
        let CoreEvent::Ready {
            protocol,
            site,
            session,
            scene,
            ..
        } = event(&out)
        else {
            panic!("expected ready, got {out}");
        };
        assert_eq!(protocol, PROTOCOL_VERSION);
        assert_eq!(s.negotiated(), Some(PROTOCOL_VERSION));
        // The site survives the wire and revalidates in the domain.
        let back = SiteFrame::try_from(&site).expect("site DTO revalidates");
        assert_eq!(Some(back), verimap_sim::demo_site().ok());
        assert_eq!(session.seed, U64String(DEMO_SEED));
        assert!(scene.replace && scene.entities.is_empty());
        assert_eq!(scene.timeline.mode, ViewModeDto::Live);
        assert_eq!(scene.timeline.tick, U64String(0));
    }

    #[test]
    fn mismatched_hello_is_incompatible_and_clears_negotiation() {
        let mut s = ready();
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor + 1));
        assert!(
            matches!(event(&out), CoreEvent::Incompatible { .. }),
            "{out}"
        );
        assert_eq!(s.negotiated(), None);
        assert_eq!(
            error_code(send(&mut s, r#"{"type":"advance","ticks":1}"#)),
            ErrorCode::HandshakeRequired
        );
    }

    #[test]
    fn requests_before_hello_are_refused() {
        let mut s = CoreSession::new();
        assert_eq!(
            error_code(send(&mut s, r#"{"type":"advance","ticks":1}"#)),
            ErrorCode::HandshakeRequired
        );
    }

    #[test]
    fn malformed_request_is_an_error_event_and_changes_nothing() {
        let mut s = CoreSession::new();
        for bad in ["", "null", "{\"type\":\"hello\"}", "\u{0}"] {
            let out = s.handle_json(bad);
            assert!(
                matches!(
                    event(&out),
                    CoreEvent::Error {
                        code: ErrorCode::MalformedRequest,
                        ..
                    }
                ),
                "{bad:?} -> {out}"
            );
            assert_eq!(s.negotiated(), None);
        }
    }

    #[test]
    fn site_wire_format_is_stable() {
        let mut s = CoreSession::new();
        let out = s.handle_json(&hello(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["type"], "ready");
        assert_eq!(v["site"]["site_id"], "demo-site-alpha");
        assert_eq!(v["site"]["anchor"]["longitude_deg"], -98.35);
        assert_eq!(v["site"]["bounds"]["x"]["min_mm"], -100_000);
        assert_eq!(v["site"]["bounds"]["z"]["max_mm"], 80_000);
        assert_eq!(v["session"]["seed"], "1");
        assert_eq!(v["scene"]["timeline"]["live_sequence"], "0");
    }

    #[test]
    fn advancing_reveals_the_vehicle_and_matches_the_native_fixture() {
        let mut s = ready();
        let mut seen = Vec::new();
        for _ in 0..3 {
            let u = update(send(&mut s, r#"{"type":"advance","ticks":100}"#));
            assert!(!u.replace && u.problem.is_none());
            seen.extend(u.entities.into_iter().map(|e| e.id));
        }
        assert!(seen.iter().all(|id| id == "trk:1"), "{seen:?}");
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/replay/demo-seed-1.json"))
                .unwrap();
        let at_300 = fixture["checkpoints"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["tick"] == 300)
            .unwrap();
        assert_eq!(state_hash(&mut s), at_300["state_hash"].as_str().unwrap());
    }

    #[test]
    fn markers_are_validated_and_edits_need_the_current_revision() {
        let mut s = ready();
        let add = r#"{"type":"add_marker","label":"Gate","at":{"position":{"x_m":10.0004,"y_m":-5,"z_m":3},"placement":"ground_plane"}}"#;
        let u = update(send(&mut s, add));
        let [marker] = u.annotations.as_slice() else {
            panic!("one marker expected: {u:?}");
        };
        assert_eq!(marker.id, "op:1");
        assert_eq!(marker.position.x_mm, 10_000);
        assert_eq!(marker.position.z_mm, 0, "ground plane forces z = 0");
        let rev = marker.revision.0;

        let outside = r#"{"type":"add_marker","label":"Far","at":{"position":{"x_m":150,"y_m":0,"z_m":0},"placement":"ground_plane"}}"#;
        assert_eq!(error_code(send(&mut s, outside)), ErrorCode::Rejected);
        let bad_label = r#"{"type":"add_marker","label":"a\nb","at":{"position":{"x_m":1,"y_m":0,"z_m":0},"placement":"ground_plane"}}"#;
        assert_eq!(error_code(send(&mut s, bad_label)), ErrorCode::InvalidField);

        let edit = |label: &str, base: u64| {
            format!(
                r#"{{"type":"edit_marker","id":"op:1","base_revision":"{base}","label":"{label}"}}"#
            )
        };
        let u = update(send(&mut s, &edit("North gate", rev)));
        assert_eq!(u.annotations[0].label, "North gate");
        assert_eq!(
            error_code(send(&mut s, &edit("Stale", rev))),
            ErrorCode::PreconditionFailed
        );
        let rev = u.annotations[0].revision.0;
        let del = format!(r#"{{"type":"delete_annotation","id":"op:1","base_revision":"{rev}"}}"#);
        let u = update(send(&mut s, &del));
        assert_eq!(u.removed_annotations, ["op:1"]);
        assert_eq!(error_code(send(&mut s, &del)), ErrorCode::Deleted);
        assert_eq!(
            error_code(send(
                &mut s,
                r#"{"type":"delete_annotation","id":"nope","base_revision":"1"}"#
            )),
            ErrorCode::InvalidField
        );
    }

    #[test]
    fn review_replays_history_without_touching_live_state() {
        let mut s = ready();
        update(send(&mut s, r#"{"type":"advance","ticks":100}"#));
        update(send(&mut s, r#"{"type":"advance","ticks":100}"#));
        let live_hash = state_hash(&mut s);

        let u = update(send(&mut s, r#"{"type":"review","tick":"100"}"#));
        assert!(u.replace);
        assert_eq!(u.timeline.mode, ViewModeDto::Review);
        assert_eq!(u.timeline.tick, U64String(100));
        assert_eq!(u.timeline.live_tick, U64String(200));
        let at_100 = state_hash(&mut s);
        assert_ne!(at_100, live_hash);

        // Read-only while reviewing.
        assert_eq!(
            error_code(send(&mut s, r#"{"type":"advance","ticks":1}"#)),
            ErrorCode::WrongMode
        );
        // Stepping forward through the journal reaches the live state.
        for _ in 0..2 {
            update(send(&mut s, r#"{"type":"review_step","ticks":60}"#));
        }
        let u = update(send(&mut s, r#"{"type":"review_step","ticks":60}"#));
        assert_eq!(u.timeline.tick, U64String(200));
        assert_eq!(state_hash(&mut s), live_hash);
        // Reviewing the same tick twice gives the same state.
        update(send(&mut s, r#"{"type":"review","tick":"100"}"#));
        assert_eq!(state_hash(&mut s), at_100);
        // A tick beyond live is clamped to live.
        let u = update(send(
            &mut s,
            r#"{"type":"review","tick":"18446744073709551615"}"#,
        ));
        assert_eq!(u.timeline.tick, U64String(200));

        let u = update(send(&mut s, r#"{"type":"go_live"}"#));
        assert_eq!(u.timeline.mode, ViewModeDto::Live);
        assert_eq!(state_hash(&mut s), live_hash);
        assert_eq!(
            error_code(send(&mut s, r#"{"type":"review_step","ticks":1}"#)),
            ErrorCode::WrongMode
        );
    }

    #[test]
    fn inspect_returns_history_and_freshness_follows_the_view_clock() {
        let mut s = ready();
        update(send(&mut s, r#"{"type":"advance","ticks":50}"#));
        let CoreEvent::EntityDetail { detail } =
            send(&mut s, r#"{"type":"inspect_entity","id":"trk:1"}"#)
        else {
            panic!("expected detail");
        };
        assert_eq!(detail.entity.id, "trk:1");
        assert!(!detail.history.is_empty() && detail.history.len() <= 16);
        assert_eq!(detail.entity.freshness, FreshnessDto::Active);
        assert_eq!(detail.entity.sigma_mm, 1_500);
        assert_eq!(
            error_code(send(&mut s, r#"{"type":"inspect_entity","id":"trk:99"}"#)),
            ErrorCode::NotFound
        );
    }
}
