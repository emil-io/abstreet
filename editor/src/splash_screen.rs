use crate::abtest::setup::PickABTest;
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{State, Transition};
use crate::mission::MissionEditMode;
use crate::sandbox::SandboxMode;
use crate::tutorial::TutorialMode;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{hotkey, Canvas, EventCtx, EventLoopMode, GfxCtx, Key, UserInput, Wizard};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::path::PathBuf;
use std::time::Instant;

pub struct SplashScreen {
    wizard: Wizard,
    maybe_screensaver: Option<(Screensaver, XorShiftRng)>,
}

impl SplashScreen {
    pub fn new_without_screensaver() -> SplashScreen {
        SplashScreen {
            wizard: Wizard::new(),
            maybe_screensaver: None,
        }
    }

    pub fn new_with_screensaver(ctx: &mut EventCtx, ui: &UI) -> SplashScreen {
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();
        SplashScreen {
            wizard: Wizard::new(),
            maybe_screensaver: Some((
                Screensaver::start_bounce(&mut rng, ctx.canvas, &ui.primary.map),
                rng,
            )),
        }
    }
}

impl State for SplashScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        if let Some((ref mut screensaver, ref mut rng)) = self.maybe_screensaver {
            screensaver.update(rng, ctx.input, ctx.canvas, &ui.primary.map);
        }

        let transition = if let Some(t) =
            splash_screen(&mut self.wizard, ctx, ui, &mut self.maybe_screensaver)
        {
            t
        } else if self.wizard.aborted() {
            Transition::Pop
        } else {
            Transition::Keep
        };
        let evloop = if self.maybe_screensaver.is_some() {
            EventLoopMode::Animation
        } else {
            EventLoopMode::InputOnly
        };

        (transition, evloop)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

const SPEED: Speed = Speed::const_meters_per_second(20.0);

struct Screensaver {
    line: Line,
    started: Instant,
}

impl Screensaver {
    fn start_bounce(rng: &mut XorShiftRng, canvas: &mut Canvas, map: &Map) -> Screensaver {
        let at = canvas.center_to_map_pt();
        let bounds = map.get_bounds();
        // TODO Ideally bounce off the edge of the map
        let goto = Pt2D::new(
            rng.gen_range(0.0, bounds.max_x),
            rng.gen_range(0.0, bounds.max_y),
        );

        canvas.cam_zoom = 10.0;
        canvas.center_on_map_pt(at);

        Screensaver {
            line: Line::new(at, goto),
            started: Instant::now(),
        }
    }

    fn update(
        &mut self,
        rng: &mut XorShiftRng,
        input: &mut UserInput,
        canvas: &mut Canvas,
        map: &Map,
    ) {
        if input.nonblocking_is_update_event() {
            input.use_update_event();
            let dist_along = Duration::seconds(elapsed_seconds(self.started)) * SPEED;
            if dist_along < self.line.length() {
                canvas.center_on_map_pt(self.line.dist_along(dist_along));
            } else {
                *self = Screensaver::start_bounce(rng, canvas, map)
            }
        }
    }
}

fn splash_screen(
    raw_wizard: &mut Wizard,
    ctx: &mut EventCtx,
    ui: &mut UI,
    maybe_screensaver: &mut Option<(Screensaver, XorShiftRng)>,
) -> Option<Transition> {
    let mut wizard = raw_wizard.wrap(ctx);
    let sandbox = "Sandbox mode";
    let load_map = "Load another map";
    let edit = "Edit map";
    let tutorial = "Tutorial";
    let debug = "Debug mode";
    let mission = "Mission Edit Mode";
    let abtest = "A/B Test Mode";
    let about = "About";
    let quit = "Quit";

    // TODO No hotkey for quit because it's just the normal menu escape?
    match wizard
        .choose_string_hotkeys(
            "Welcome to A/B Street!",
            vec![
                (hotkey(Key::S), sandbox),
                (hotkey(Key::L), load_map),
                (hotkey(Key::E), edit),
                (hotkey(Key::T), tutorial),
                (hotkey(Key::D), debug),
                (hotkey(Key::M), mission),
                (hotkey(Key::A), abtest),
                (None, about),
                (None, quit),
            ],
        )?
        .as_str()
    {
        x if x == sandbox => Some(Transition::Push(Box::new(SandboxMode::new(ctx)))),
        x if x == load_map => {
            let current_map = ui.primary.map.get_name().to_string();
            if let Some((name, _)) = wizard.choose_something_no_keys::<String>(
                "Load which map?",
                Box::new(move || {
                    abstutil::list_all_objects("maps", "")
                        .into_iter()
                        .filter(|(n, _)| n != &current_map)
                        .collect()
                }),
            ) {
                // This retains no state, but that's probably fine.
                let mut flags = ui.primary.current_flags.clone();
                flags.sim_flags.load = PathBuf::from(format!("../data/maps/{}.bin", name));
                *ui = UI::new(flags, ctx, false);
                // TODO want to clear wizard and screensaver as we leave this state.
                Some(Transition::Push(Box::new(SandboxMode::new(ctx))))
            } else if wizard.aborted() {
                Some(Transition::Replace(Box::new(SplashScreen {
                    wizard: Wizard::new(),
                    maybe_screensaver: maybe_screensaver.take(),
                })))
            } else {
                None
            }
        }
        x if x == edit => Some(Transition::Push(Box::new(EditMode::new(ctx, ui)))),
        x if x == tutorial => Some(Transition::Push(Box::new(TutorialMode::new(ctx, ui)))),
        x if x == debug => Some(Transition::Push(Box::new(DebugMode::new(ctx, ui)))),
        x if x == mission => Some(Transition::Push(Box::new(MissionEditMode::new(ctx, ui)))),
        x if x == abtest => Some(Transition::Push(Box::new(PickABTest::new()))),
        x if x == about => {
            if wizard.acknowledge(
                "About A/B Street",
                vec![
                    "Author: Dustin Carlino (dabreegster@gmail.com)",
                    "http://github.com/dabreegster/abstreet",
                    "Map data from OpenStreetMap and King County GIS",
                    "",
                    "Press ENTER to continue",
                ],
            ) {
                Some(Transition::Replace(Box::new(SplashScreen {
                    wizard: Wizard::new(),
                    maybe_screensaver: maybe_screensaver.take(),
                })))
            } else {
                None
            }
        }
        x if x == quit => Some(Transition::Pop),
        _ => unreachable!(),
    }
}
