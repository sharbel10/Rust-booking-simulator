mod api;

use api::Snapshot;
use leptos::prelude::*;
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;

type Labels = (&'static str, &'static str);

fn text(arabic: bool, labels: Labels) -> &'static str {
    if arabic { labels.1 } else { labels.0 }
}

fn update(
    action: Option<Value>,
    snapshot: RwSignal<Option<Snapshot>>,
    failed: RwSignal<bool>,
    busy: RwSignal<bool>,
) {
    if busy.get_untracked() {
        return;
    }
    busy.set(true);
    spawn_local(async move {
        match api::request(action).await {
            Ok(next) => {
                snapshot.set(Some(next));
                failed.set(false);
            }
            Err(()) => failed.set(true),
        }
        busy.set(false);
    });
}

fn status_label(code: &str, arabic: bool) -> &str {
    let labels = match code.split('(').next().unwrap_or(code) {
        "Requested" => ("Request received", "الطلب مستلم"),
        "AwaitingFunding" => ("Awaiting funding", "بانتظار التمويل"),
        "Confirmed" => ("Confirmed", "مؤكّد"),
        "InProgress" => ("In progress", "قيد التنفيذ"),
        "Settling" => ("Awaiting payout", "بانتظار الصرف"),
        "Completed" => ("Completed", "مكتمل"),
        "Cancelled" => ("Cancelled", "ملغى"),
        "Pending" => ("Not started", "لم تبدأ"),
        "Submitted" => ("Ready for review", "جاهزة للمراجعة"),
        "Approved" => ("Approved", "معتمدة"),
        "Unfunded" => ("Not funded", "غير مموّل"),
        "Held" => ("Held in escrow", "محفوظ لدى الوسيط"),
        "Released" => ("Paid to videographer", "انصرف للمصوّر"),
        "PayoutPending" => ("Payout pending", "الصرف قيد التأكيد"),
        _ => return code,
    };
    text(arabic, labels)
}

struct NextAction {
    label: Labels,
    hint: Labels,
    request: Value,
}

impl NextAction {
    fn new(label: Labels, hint: Labels, request: Value) -> Self {
        Self {
            label,
            hint,
            request,
        }
    }
}

fn next_action(snapshot: &Snapshot) -> Option<NextAction> {
    match snapshot.state.as_str() {
        "Requested" => Some(NextAction::new(
            ("Accept booking", "اقبل الحجز"),
            (
                "The videographer accepts the agency’s request.",
                "المصوّر بيقبل طلب الوكالة.",
            ),
            json!({"action":"accept"}),
        )),
        "AwaitingFunding" => Some(NextAction::new(
            ("Confirm funding", "أكّد التمويل"),
            (
                "Simulate the escrow provider confirming the AED 1,000 hold.",
                "حاكي تأكيد الوسيط إن الـ١٠٠٠ درهم محفوظة.",
            ),
            json!({"action":"simulate_funding"}),
        )),
        "Confirmed" | "InProgress" => {
            let (index, stage) = snapshot
                .milestones
                .iter()
                .enumerate()
                .find(|(_, state)| !state.starts_with("Approved"))?;
            let delivery = index + 1;
            let stage = stage.split('(').next()?;
            let raw = index == 0;
            let label = match (stage, raw) {
                ("Pending", true) => ("Start shooting", "ابدأ التصوير"),
                ("Pending", false) => ("Start editing", "ابدأ المونتاج"),
                ("InProgress", true) => ("Submit raw footage", "سلّم اللقطات الخام"),
                ("InProgress", false) => ("Submit final video", "سلّم الفيديو النهائي"),
                ("Submitted", true) => ("Approve raw footage", "اعتمد اللقطات الخام"),
                ("Submitted", false) => ("Approve final video", "اعتمد الفيديو النهائي"),
                _ => return None,
            };
            let hint = match stage {
                "Pending" => (
                    "The videographer starts the next milestone.",
                    "المصوّر بيبلّش المرحلة الجاية.",
                ),
                "InProgress" => (
                    "The videographer submits a version for the agency to review.",
                    "المصوّر بيسلّم نسخة حتى الوكالة تراجعها.",
                ),
                "Submitted" if raw => (
                    "The agency approves the footage. The funds stay held while editing is still open.",
                    "الوكالة بتعتمد اللقطات. المصاري بتضل محفوظة والمونتاج لسا مفتوح.",
                ),
                _ => (
                    "The agency approves the final video and the booking moves to payout.",
                    "الوكالة بتعتمد الفيديو النهائي والحجز بينتقل لانتظار الصرف.",
                ),
            };
            let action = match stage {
                "Pending" => "start",
                "InProgress" => "submit",
                _ => "approve",
            };
            let request = if action == "start" {
                json!({"action": action, "index": index})
            } else {
                json!({"action": action, "index": index, "delivery": delivery})
            };
            Some(NextAction::new(label, hint, request))
        }
        "Settling" => Some(NextAction::new(
            ("Confirm payout", "أكّد الصرف"),
            (
                "Simulate the escrow provider confirming payment to the videographer.",
                "حاكي تأكيد الوسيط إن المصاري تحوّلت للمصوّر.",
            ),
            json!({"action":"simulate_payout"}),
        )),
        _ => None,
    }
}

fn app() -> impl IntoView {
    let arabic = RwSignal::new(false);
    let snapshot = RwSignal::new(None::<Snapshot>);
    let failed = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let label = move |en, ar| text(arabic.get(), (en, ar));
    let send = move |action| update(action, snapshot, failed, busy);
    send(None);

    view! {
        <main class="shell" dir=move || if arabic.get() { "rtl" } else { "ltr" }
              lang=move || if arabic.get() { "ar" } else { "en" }>
            <header class="page-header">
                <div>
                    <p class="eyebrow">{move || label("Booking simulator", "محاكاة الحجز")}</p>
                    <h1>{move || label("Videographer booking", "حجز المصوّر")}</h1>
                    <p class="subtitle">{move || label("An agency books a shoot and a final edit.", "وكالة حجزت تصوير ومونتاج فيديو نهائي.")}</p>
                </div>
                <button class="language" type="button" on:click=move |_| arabic.update(|value| *value = !*value)>
                    {move || label("العربية", "English")}
                </button>
            </header>
            <Show when=move || failed.get()>
                <div class="feedback" role="alert">
                    {move || label("Couldn’t reach the booking server.", "تعذّر الاتصال بسيرفر الحجز.")}
                    <button class="quiet" type="button" on:click=move |_| send(None)>
                        {move || label("Retry", "إعادة المحاولة")}
                    </button>
                </div>
            </Show>
            <Show when=move || snapshot.get().is_none() && !failed.get()>
                <p role="status">{move || label("Loading booking…", "عم يتحمّل الحجز…")}</p>
            </Show>
            {move || snapshot.get().map(|current| {
                let is_arabic = arabic.get();
                let next = next_action(&current);
                let early = matches!(current.state.as_str(), "Requested" | "AwaitingFunding");
                let can_advance = current.escrow == "Held"
                    && matches!(current.state.as_str(), "Confirmed" | "InProgress")
                    && current.hour < 49;
                let feedback = current.error.as_deref().map(|error| {
                    if early && error == "BookingNotReadyForMilestone" {
                        label("Start rejected. Confirm funding before work begins.", "تم رفض البدء. لازم يتأكد التمويل قبل الشغل.")
                    } else {
                        label("This action is unavailable in the current state.", "هالإجراء غير متاح بالحالة الحالية.")
                    }
                });
                view! {
                    <div class="layout">
                        <div class="workflow">
                            <section class="panel" aria-live="polite">
                                <div class="panel-head">
                                    <h2>{label("Milestones", "مراحل الشغل")}</h2>
                                    <span class="count">{format!("{}/2", current.milestones.iter().filter(|state| state.starts_with("Approved")).count())}</span>
                                </div>
                                <ol class="milestones">
                                    {current.milestones.into_iter().enumerate().map(|(index, state)| {
                                        let name = if index == 0 { label("Raw footage", "اللقطات الخام") }
                                            else { label("Final video", "الفيديو النهائي") };
                                        let approved = state.starts_with("Approved");
                                        view! {
                                            <li>
                                                <span class="step" class:done=approved>{if approved { "✓".into() } else { (index + 1).to_string() }}</span>
                                                <strong>{name}</strong>
                                                <span class="stage-status">{status_label(&state, is_arabic).to_owned()}</span>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ol>
                            </section>
                            <section class="panel action-panel" aria-live="polite">
                                {next.map(|next| view! {
                                    <h2>{label("Next action", "الخطوة الجاية")}</h2>
                                    <p>{text(is_arabic, next.hint)}</p>
                                    <button class="primary" type="button" disabled=move || busy.get()
                                        on:click=move |_| send(Some(next.request.clone()))>
                                        {text(is_arabic, next.label)}
                                    </button>
                                }.into_any()).unwrap_or_else(|| view! {
                                    <h2>{label("Booking complete", "الحجز اكتمل")}</h2>
                                    <p>{label("The simulated provider confirmed the payout.", "الوسيط المحاكى أكّد الصرف للمصوّر.")}</p>
                                    <button class="primary" type="button" disabled=move || busy.get()
                                        on:click=move |_| send(Some(json!({"action":"reset"})))>
                                        {label("Run another demo", "أعد التجربة")}
                                    </button>
                                }.into_any())}
                                {feedback.map(|reason| view! { <p class="feedback" role="alert">{reason}</p> })}
                            </section>
                        </div>
                        <aside class="summary" aria-label=label("Booking information", "معلومات الحجز")>
                            <section class="panel summary-panel" aria-live="polite">
                                <h2>{label("Booking status", "حالة الحجز")}</h2>
                                <p class="booking-status">{status_label(&current.state, is_arabic).to_owned()}</p>
                                <dl>
                                    <div><dt>{label("Booking total", "المبلغ الإجمالي")}</dt>
                                         <dd>{label("AED 1,000", "١٠٠٠ درهم")}</dd></div>
                                    <div><dt>{label("Payment status", "حالة المبلغ")}</dt>
                                         <dd>{status_label(&current.escrow, is_arabic).to_owned()}</dd></div>
                                    <div><dt>{label("Simulation hour", "ساعة المحاكاة")}</dt><dd>{current.hour}</dd></div>
                                    {current.expires_at_hour.map(|expiry| view! {
                                        <div><dt>{label("Hold expires at hour", "ينتهي حجز المبلغ بالساعة")}</dt><dd>{expiry}</dd></div>
                                    })}
                                </dl>
                            </section>
                            <section class="panel demo-panel">
                                <h2>{label("Demo controls", "خيارات المحاكاة")}</h2>
                                {early.then(|| view! {
                                    <button class="quiet" type="button" disabled=move || busy.get()
                                        on:click=move |_| send(Some(json!({"action":"start","index":0})))>
                                        {label("Try starting before funding", "جرّب البدء قبل التمويل")}
                                    </button>
                                })}
                                {can_advance.then(|| view! {
                                    <button class="quiet" type="button" disabled=move || busy.get()
                                        on:click=move |_| send(Some(json!({"action":"advance_to_hour49"})))>
                                        {label("Advance to hour 49", "قدّم الوقت للساعة ٤٩")}
                                    </button>
                                })}
                                <button class="quiet" type="button" disabled=move || busy.get()
                                    on:click=move |_| send(Some(json!({"action":"reset"})))>
                                    {label("Reset demo", "إعادة المحاكاة")}
                                </button>
                            </section>
                        </aside>
                    </div>
                }
            })}
        </main>
    }
}

fn main() {
    leptos::mount::mount_to_body(app);
}
