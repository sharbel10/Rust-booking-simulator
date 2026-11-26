use crate::BookingState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hold {
    pub id: u64,
    pub expires_at_hour: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowState {
    Unfunded,
    Held(Hold),
    PayoutPending(Hold),
    Released(Hold),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewalRequest {
    pub hold_id: u64,
    pub previous_expiry: u64,
    pub requested_expiry: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
    HoldExpired,
    HoldUnavailable,
    UnexpectedConfirmation,
}

pub(crate) fn renewal_due(
    state: BookingState,
    escrow: EscrowState,
    now_hour: u64,
    renew_within_hours: u64,
    extend_by_hours: u64,
) -> Option<RenewalRequest> {
    if !matches!(
        state,
        BookingState::Confirmed | BookingState::InProgress | BookingState::Settling
    ) {
        return None;
    }
    let EscrowState::Held(hold) = escrow else {
        return None;
    };
    let remaining = hold.expires_at_hour.checked_sub(now_hour)?;
    if remaining == 0 || remaining > renew_within_hours || extend_by_hours == 0 {
        return None;
    }
    Some(RenewalRequest {
        hold_id: hold.id,
        previous_expiry: hold.expires_at_hour,
        requested_expiry: hold.expires_at_hour.checked_add(extend_by_hours)?,
    })
}
