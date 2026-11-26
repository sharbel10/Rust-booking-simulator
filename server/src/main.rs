mod postgres;

use postgres::PostgresStore;
use std::{num::NonZeroUsize, sync::Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use booking_core::{
    Booking, BookingEvent, DeliveryId, EscrowState, Hold, MilestoneEvent, RenewalRequest,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

#[derive(Debug, Clone)]
struct Demo {
    booking: Booking,
    hour: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
enum Action {
    Reset,
    Accept,
    SimulateFunding,
    Start { index: usize },
    Submit { index: usize, delivery: u32 },
    Approve { index: usize, delivery: u32 },
    AdvanceToHour49,
    SimulatePayout,
}

enum ConfirmedOperation {
    Renewal(RenewalRequest),
    Payout(u64),
}

#[derive(Serialize)]
struct Snapshot {
    state: String,
    milestones: Vec<String>,
    escrow: String,
    expires_at_hour: Option<u64>,
    hour: u64,
    error: Option<String>,
}

impl Demo {
    fn new() -> Self {
        Self {
            booking: Booking::new(NonZeroUsize::new(2).unwrap()),
            hour: 0,
        }
    }

    fn snapshot(&self, error: Option<String>) -> Snapshot {
        let escrow = self.booking.escrow();
        let (escrow_name, expires_at_hour) = match escrow {
            EscrowState::Unfunded => ("Unfunded", None),
            EscrowState::Held(hold) => ("Held", Some(hold.expires_at_hour)),
            EscrowState::PayoutPending(_) => ("PayoutPending", None),
            EscrowState::Released(_) => ("Released", None),
        };

        Snapshot {
            state: format!("{:?}", self.booking.state()),
            milestones: self
                .booking
                .milestones()
                .iter()
                .map(|state| format!("{state:?}"))
                .collect(),
            escrow: escrow_name.into(),
            expires_at_hour,
            hour: self.hour,
            error,
        }
    }

    fn apply(
        &mut self,
        action: Action,
    ) -> Result<Option<ConfirmedOperation>, booking_core::BookingError> {
        match action {
            Action::Reset => *self = Self::new(),
            Action::Accept => self.booking.apply_event(BookingEvent::ProviderAccepted)?,
            Action::SimulateFunding => {
                self.booking.confirm_funding(
                    Hold {
                        id: 1,
                        expires_at_hour: self.hour + 50,
                    },
                    self.hour,
                )?;
            }
            Action::Start { index } => self
                .booking
                .apply_milestone_event(index, MilestoneEvent::Start)?,
            Action::Submit { index, delivery } => {
                self.booking.apply_milestone_event(
                    index,
                    MilestoneEvent::Submit(DeliveryId(u64::from(delivery))),
                )?;
            }
            Action::Approve { index, delivery } => {
                self.booking.apply_milestone_event(
                    index,
                    MilestoneEvent::Approve(DeliveryId(u64::from(delivery))),
                )?;
            }
            Action::AdvanceToHour49 => {
                self.hour = 49;
                if let Some(request) = self.booking.renewal_due(self.hour, 2, 24) {
                    // The demo provider immediately confirms a valid renewal.
                    self.booking.confirm_renewal(request)?;
                    return Ok(Some(ConfirmedOperation::Renewal(request)));
                }
            }
            Action::SimulatePayout => {
                let hold_id = self.booking.request_payout(self.hour)?;
                self.booking.confirm_payout(hold_id)?;
                return Ok(Some(ConfirmedOperation::Payout(hold_id)));
            }
        }
        Ok(None)
    }
}

#[derive(Clone)]
enum AppState {
    Memory(std::sync::Arc<Mutex<Demo>>),
    Postgres(PostgresStore),
}

async fn snapshot(State(state): State<AppState>) -> Result<Json<Snapshot>, StatusCode> {
    match state {
        AppState::Memory(demo) => Ok(Json(demo.lock().unwrap().snapshot(None))),
        AppState::Postgres(store) => store.snapshot().await.map(Json).map_err(database_error),
    }
}

async fn action(
    State(state): State<AppState>,
    Json(action): Json<Action>,
) -> Result<(StatusCode, Json<Snapshot>), StatusCode> {
    let snapshot = match state {
        AppState::Memory(demo) => {
            let mut demo = demo.lock().unwrap();
            let mut next = demo.clone();
            match next.apply(action) {
                Ok(_) => {
                    *demo = next;
                    demo.snapshot(None)
                }
                Err(error) => demo.snapshot(Some(format!("{error:?}"))),
            }
        }
        AppState::Postgres(store) => store.apply(action).await.map_err(database_error)?,
    };
    let status = if snapshot.error.is_some() {
        StatusCode::CONFLICT
    } else {
        StatusCode::OK
    };
    Ok((status, Json(snapshot)))
}

fn database_error(error: sqlx::Error) -> StatusCode {
    eprintln!("Booking database request failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            let store = PostgresStore::connect(&url).await?;
            println!("Booking storage: PostgreSQL");
            AppState::Postgres(store)
        }
        Err(std::env::VarError::NotPresent) => {
            println!("Booking storage: in memory");
            AppState::Memory(std::sync::Arc::new(Mutex::new(Demo::new())))
        }
        Err(error) => return Err(error.into()),
    };
    let web_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../desktop/ui/dist");
    let app = Router::new()
        .route("/booking", get(snapshot))
        .route("/booking/action", post(action))
        .fallback_service(ServeDir::new(web_root))
        .with_state(state);
    let port = std::env::var("BOOKING_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse::<u16>()?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("Booking simulator: http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
