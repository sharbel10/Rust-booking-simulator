use crate::booking::{BookingEvent, BookingState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBookingTransition {
    pub state: BookingState,
    pub event: BookingEvent,
}

pub fn transition(
    state: BookingState,
    event: BookingEvent,
) -> Result<BookingState, InvalidBookingTransition> {
    use BookingEvent::*;
    use BookingState::*;

    match (state, event) {
        (Requested, ProviderAccepted) => Ok(AwaitingFunding),
        (AwaitingFunding, FundingConfirmed) => Ok(Confirmed),
        (Confirmed, WorkStarted) => Ok(InProgress),
        (InProgress, FinalMilestoneApproved) => Ok(Settling),
        (Settling, PayoutConfirmed) => Ok(Completed),
        (Requested, BuyerWithdrew | ProviderDeclined) => Ok(Cancelled),
        _ => Err(InvalidBookingTransition { state, event }),
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidBookingTransition, transition};
    use crate::{BookingEvent::*, BookingState::*};

    #[test]
    fn booking_completes_only_after_payout_confirmation() {
        let mut state = Requested;
        for (event, expected) in [
            (ProviderAccepted, AwaitingFunding),
            (FundingConfirmed, Confirmed),
            (WorkStarted, InProgress),
            (FinalMilestoneApproved, Settling),
            (PayoutConfirmed, Completed),
        ] {
            state = transition(state, event).unwrap();
            assert_eq!(state, expected);
        }
    }

    #[test]
    fn rejects_work_before_funding_and_payout_before_approval() {
        for (state, event) in [
            (AwaitingFunding, WorkStarted),
            (InProgress, PayoutConfirmed),
        ] {
            assert_eq!(
                transition(state, event),
                Err(InvalidBookingTransition { state, event }),
            );
        }
    }

    #[test]
    fn cancelled_booking_cannot_be_reopened_by_funding_confirmation() {
        let state = transition(Requested, BuyerWithdrew).unwrap();
        assert_eq!(state, Cancelled);
        assert_eq!(
            transition(state, FundingConfirmed),
            Err(InvalidBookingTransition {
                state,
                event: FundingConfirmed,
            }),
        );
    }
}
