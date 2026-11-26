use std::path::Path;

use booking_core::{Booking, BookingState, DeliveryId, EscrowState, Hold, MilestoneState};
use sqlx::{PgPool, Postgres, Transaction, migrate::Migrator, postgres::PgPoolOptions};

use crate::{Action, ConfirmedOperation, Demo, Snapshot};

const BOOKING_ID: i32 = 1;

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;
        let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        Migrator::new(migrations.as_path())
            .await?
            .run(&pool)
            .await?;
        Ok(Self { pool })
    }

    pub async fn snapshot(&self) -> Result<Snapshot, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let demo = load(&mut tx).await?;
        tx.commit().await?;
        Ok(demo.snapshot(None))
    }

    pub async fn apply(&self, action: Action) -> Result<Snapshot, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let demo = load(&mut tx).await?;
        let mut next = demo.clone();
        let operation = match next.apply(action) {
            Ok(operation) => operation,
            Err(error) => return Ok(demo.snapshot(Some(format!("{error:?}")))),
        };
        save(&mut tx, &next).await?;
        if let Some(operation) = operation {
            record_operation(&mut tx, operation).await?;
        }
        tx.commit().await?;
        Ok(next.snapshot(None))
    }
}

async fn load(tx: &mut Transaction<'_, Postgres>) -> Result<Demo, sqlx::Error> {
    // Lock the parent row before reading or updating the related rows.
    let (state, hour): (String, i64) =
        sqlx::query_as("SELECT state, hour FROM bookings WHERE id = $1 FOR UPDATE")
            .bind(BOOKING_ID)
            .fetch_one(&mut **tx)
            .await?;

    let rows: Vec<(i32, String, Option<i64>)> = sqlx::query_as(
        "SELECT position, state, delivery_id FROM milestones WHERE booking_id = $1 ORDER BY position",
    )
    .bind(BOOKING_ID)
    .fetch_all(&mut **tx)
    .await?;
    let milestones = rows
        .into_iter()
        .enumerate()
        .map(|(index, (position, state, delivery))| {
            if usize::try_from(position).ok() != Some(index) {
                return Err(bad_data("invalid milestone order"));
            }
            let delivery = delivery.map(from_bigint).transpose()?.map(DeliveryId);
            match (state.as_str(), delivery) {
                ("Pending", None) => Ok(MilestoneState::Pending),
                ("InProgress", None) => Ok(MilestoneState::InProgress),
                ("Submitted", Some(delivery)) => Ok(MilestoneState::Submitted(delivery)),
                ("Approved", Some(delivery)) => Ok(MilestoneState::Approved(delivery)),
                _ => Err(bad_data("invalid milestone state")),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let row: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT hold_id, state, expires_at_hour FROM escrow_holds WHERE booking_id = $1",
    )
    .bind(BOOKING_ID)
    .fetch_optional(&mut **tx)
    .await?;
    let escrow = if let Some((id, status, expiry)) = row {
        let hold = Hold {
            id: from_bigint(id)?,
            expires_at_hour: from_bigint(expiry)?,
        };
        match status.as_str() {
            "Held" => EscrowState::Held(hold),
            "PayoutPending" => EscrowState::PayoutPending(hold),
            "Released" => EscrowState::Released(hold),
            _ => return Err(bad_data("invalid escrow state")),
        }
    } else {
        EscrowState::Unfunded
    };

    let state = match state.as_str() {
        "Requested" => BookingState::Requested,
        "AwaitingFunding" => BookingState::AwaitingFunding,
        "Confirmed" => BookingState::Confirmed,
        "InProgress" => BookingState::InProgress,
        "Settling" => BookingState::Settling,
        "Completed" => BookingState::Completed,
        "Cancelled" => BookingState::Cancelled,
        _ => return Err(bad_data("invalid booking state")),
    };
    let booking = Booking::restore(state, milestones, escrow)
        .ok_or_else(|| bad_data("inconsistent booking, milestone, and escrow states"))?;
    Ok(Demo {
        booking,
        hour: from_bigint(hour)?,
    })
}

async fn save(tx: &mut Transaction<'_, Postgres>, demo: &Demo) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE bookings SET state = $2, hour = $3 WHERE id = $1")
        .bind(BOOKING_ID)
        .bind(format!("{:?}", demo.booking.state()))
        .bind(to_bigint(demo.hour)?)
        .execute(&mut **tx)
        .await?;

    for (index, state) in demo.booking.milestones().iter().enumerate() {
        let (status, delivery) = match state {
            MilestoneState::Pending => ("Pending", None),
            MilestoneState::InProgress => ("InProgress", None),
            MilestoneState::Submitted(delivery) => ("Submitted", Some(to_bigint(delivery.0)?)),
            MilestoneState::Approved(delivery) => ("Approved", Some(to_bigint(delivery.0)?)),
        };
        sqlx::query(
            "UPDATE milestones SET state = $3, delivery_id = $4 WHERE booking_id = $1 AND position = $2",
        )
        .bind(BOOKING_ID)
        .bind(i32::try_from(index).map_err(|_| bad_data("milestone index too large"))?)
        .bind(status)
        .bind(delivery)
        .execute(&mut **tx)
        .await?;
    }

    let (status, hold) = match demo.booking.escrow() {
        EscrowState::Held(hold) => ("Held", hold),
        EscrowState::PayoutPending(hold) => ("PayoutPending", hold),
        EscrowState::Released(hold) => ("Released", hold),
        EscrowState::Unfunded => {
            sqlx::query(
                "DELETE FROM escrow_operations WHERE hold_id IN
                    (SELECT hold_id FROM escrow_holds WHERE booking_id = $1)",
            )
            .bind(BOOKING_ID)
            .execute(&mut **tx)
            .await?;
            sqlx::query("DELETE FROM escrow_holds WHERE booking_id = $1")
                .bind(BOOKING_ID)
                .execute(&mut **tx)
                .await?;
            return Ok(());
        }
    };
    sqlx::query(
        "INSERT INTO escrow_holds (booking_id, hold_id, state, expires_at_hour, amount_minor, currency)
         VALUES ($1, $2, $3, $4, 100000, 'AED')
         ON CONFLICT (booking_id) DO UPDATE SET
             hold_id = EXCLUDED.hold_id, state = EXCLUDED.state, expires_at_hour = EXCLUDED.expires_at_hour",
    )
    .bind(BOOKING_ID)
    .bind(to_bigint(hold.id)?)
    .bind(status)
    .bind(to_bigint(hold.expires_at_hour)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn record_operation(
    tx: &mut Transaction<'_, Postgres>,
    operation: ConfirmedOperation,
) -> Result<(), sqlx::Error> {
    let (hold_id, kind, previous, confirmed) = match operation {
        ConfirmedOperation::Renewal(request) => (
            request.hold_id,
            "renewal",
            Some(to_bigint(request.previous_expiry)?),
            Some(to_bigint(request.requested_expiry)?),
        ),
        ConfirmedOperation::Payout(hold_id) => (hold_id, "payout", None, None),
    };
    sqlx::query(
        "INSERT INTO escrow_operations (hold_id, kind, previous_expiry, confirmed_expiry)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(to_bigint(hold_id)?)
    .bind(kind)
    .bind(previous)
    .bind(confirmed)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn bad_data(message: &str) -> sqlx::Error {
    sqlx::Error::Protocol(message.into())
}

fn to_bigint(value: u64) -> Result<i64, sqlx::Error> {
    i64::try_from(value).map_err(|_| bad_data("value too large for PostgreSQL bigint"))
}

fn from_bigint(value: i64) -> Result<u64, sqlx::Error> {
    u64::try_from(value).map_err(|_| bad_data("negative value in persisted booking"))
}
