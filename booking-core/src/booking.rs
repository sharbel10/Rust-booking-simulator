#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingState {
    Requested,
    AwaitingFunding,
    Confirmed,
    InProgress,
    Settling,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingEvent {
    ProviderAccepted,
    FundingConfirmed,
    WorkStarted,
    FinalMilestoneApproved,
    PayoutConfirmed,
    BuyerWithdrew,
    ProviderDeclined,
}

use std::num::NonZeroUsize;

use crate::escrow::{EscrowError, EscrowState, Hold, RenewalRequest, renewal_due};
use crate::milestone::{MilestoneError, MilestoneEvent, MilestoneState, transition_milestone};
use crate::service::{InvalidBookingTransition, transition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Booking {
    state: BookingState,
    milestones: Vec<MilestoneState>,
    escrow: EscrowState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingError {
    InvalidTransition(InvalidBookingTransition),
    Milestone(MilestoneError),
    MilestoneNotFound,
    MilestoneOutOfOrder,
    BookingNotReadyForMilestone,
    DerivedEventRequired,
    Escrow(EscrowError),
}

impl Booking {
    /// ينشئ حجزاً جديداً بحالة Requested، وكل مراحله بحالة Pending.
    pub fn new(milestone_count: NonZeroUsize) -> Self {
        Self {
            state: BookingState::Requested,
            escrow: EscrowState::Unfunded,
            milestones: vec![MilestoneState::Pending; milestone_count.get()],
        }
    }

    /// يرجع حجزاً محفوظاً بعد التحقق من توافق حالته مع المراحل والمبلغ.
    pub fn restore(
        state: BookingState,
        milestones: Vec<MilestoneState>,
        escrow: EscrowState,
    ) -> Option<Self> {
        if milestones.is_empty() {
            return None;
        }

        let approved = milestones
            .iter()
            .take_while(|stage| matches!(stage, MilestoneState::Approved(_)))
            .count();
        let remaining = &milestones[approved..];
        let ordered = remaining
            .iter()
            .skip(1)
            .all(|stage| *stage == MilestoneState::Pending);
        let untouched = approved == 0
            && milestones
                .iter()
                .all(|stage| *stage == MilestoneState::Pending);
        let all_approved = remaining.is_empty();
        let valid = match state {
            BookingState::Requested | BookingState::AwaitingFunding | BookingState::Cancelled => {
                untouched && escrow == EscrowState::Unfunded
            }
            BookingState::Confirmed => untouched && matches!(escrow, EscrowState::Held(_)),
            BookingState::InProgress => {
                ordered && !untouched && !all_approved && matches!(escrow, EscrowState::Held(_))
            }
            BookingState::Settling => {
                all_approved
                    && matches!(escrow, EscrowState::Held(_) | EscrowState::PayoutPending(_))
            }
            BookingState::Completed => all_approved && matches!(escrow, EscrowState::Released(_)),
        };
        valid.then_some(Self {
            state,
            milestones,
            escrow,
        })
    }

    /// يرجع الحالة الحالية للحجز ككل.
    pub fn state(&self) -> BookingState {
        self.state
    }

    /// يعرض حالات المراحل بالترتيب للقراءة فقط.
    pub fn milestones(&self) -> &[MilestoneState] {
        &self.milestones
    }

    /// يرجع حالة المبلغ الحالي لدى وسيط الدفع.
    pub fn escrow(&self) -> EscrowState {
        self.escrow
    }

    pub fn confirm_funding(&mut self, hold: Hold, now_hour: u64) -> Result<(), BookingError> {
        let next = transition(self.state, BookingEvent::FundingConfirmed)
            .map_err(BookingError::InvalidTransition)?;
        if hold.expires_at_hour <= now_hour {
            return Err(BookingError::Escrow(EscrowError::HoldExpired));
        }
        self.escrow = EscrowState::Held(hold);
        self.state = next;
        Ok(())
    }

    pub fn renewal_due(
        &self,
        now_hour: u64,
        renew_within_hours: u64,
        extend_by_hours: u64,
    ) -> Option<RenewalRequest> {
        renewal_due(
            self.state,
            self.escrow,
            now_hour,
            renew_within_hours,
            extend_by_hours,
        )
    }

    pub fn confirm_renewal(&mut self, request: RenewalRequest) -> Result<(), BookingError> {
        let EscrowState::Held(hold) = self.escrow else {
            return Err(BookingError::Escrow(EscrowError::UnexpectedConfirmation));
        };
        if hold.id != request.hold_id
            || hold.expires_at_hour != request.previous_expiry
            || request.requested_expiry <= request.previous_expiry
        {
            return Err(BookingError::Escrow(EscrowError::UnexpectedConfirmation));
        }
        self.escrow = EscrowState::Held(Hold {
            id: hold.id,
            expires_at_hour: request.requested_expiry,
        });
        Ok(())
    }

    pub fn request_payout(&mut self, now_hour: u64) -> Result<u64, BookingError> {
        transition(self.state, BookingEvent::PayoutConfirmed)
            .map_err(BookingError::InvalidTransition)?;
        let EscrowState::Held(hold) = self.escrow else {
            return Err(BookingError::Escrow(EscrowError::HoldUnavailable));
        };
        if hold.expires_at_hour <= now_hour {
            return Err(BookingError::Escrow(EscrowError::HoldExpired));
        }
        self.escrow = EscrowState::PayoutPending(hold);
        Ok(hold.id)
    }

    pub fn confirm_payout(&mut self, hold_id: u64) -> Result<(), BookingError> {
        let EscrowState::PayoutPending(hold) = self.escrow else {
            return Err(BookingError::Escrow(EscrowError::UnexpectedConfirmation));
        };
        if hold.id != hold_id {
            return Err(BookingError::Escrow(EscrowError::UnexpectedConfirmation));
        }
        self.state = transition(self.state, BookingEvent::PayoutConfirmed)
            .map_err(BookingError::InvalidTransition)?;
        self.escrow = EscrowState::Released(hold);
        Ok(())
    }

    /// يطبّق حدثاً على الحجز، مثل قبول الطلب أو سحبه.
    pub fn apply_event(&mut self, event: BookingEvent) -> Result<(), BookingError> {
        if matches!(
            event,
            BookingEvent::WorkStarted
                | BookingEvent::FinalMilestoneApproved
                | BookingEvent::FundingConfirmed
                | BookingEvent::PayoutConfirmed
        ) {
            return Err(BookingError::DerivedEventRequired);
        }

        self.state = transition(self.state, event).map_err(BookingError::InvalidTransition)?;
        Ok(())
    }

    /// يطبّق حدثاً على مرحلة محددة ويحدّث حالة الحجز إذا بدأت أول مرحلة أو اعتُمدت الأخيرة.
    pub fn apply_milestone_event(
        &mut self,
        index: usize,
        event: MilestoneEvent,
    ) -> Result<(), BookingError> {
        let current = *self
            .milestones
            .get(index)
            .ok_or(BookingError::MilestoneNotFound)?;
        if !matches!(
            self.state,
            BookingState::Confirmed | BookingState::InProgress
        ) {
            return Err(BookingError::BookingNotReadyForMilestone);
        }
        if !self.milestones[..index]
            .iter()
            .all(|state| matches!(state, MilestoneState::Approved(_)))
        {
            return Err(BookingError::MilestoneOutOfOrder);
        }

        let next_milestone =
            transition_milestone(current, event).map_err(BookingError::Milestone)?;
        let next_state = match (self.state, next_milestone) {
            (BookingState::Confirmed, MilestoneState::InProgress) => {
                transition(self.state, BookingEvent::WorkStarted)
            }
            (BookingState::InProgress, MilestoneState::Approved(_))
                if index == self.milestones.len() - 1 =>
            {
                transition(self.state, BookingEvent::FinalMilestoneApproved)
            }
            _ => Ok(self.state),
        }
        .map_err(BookingError::InvalidTransition)?;

        self.milestones[index] = next_milestone;
        self.state = next_state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeliveryId;

    #[test]
    // يتحقق من ترتيب المراحل وأن اعتماد الأخيرة ينقل الحجز إلى انتظار الصرف.
    fn only_the_last_milestone_moves_the_booking_to_settling() {
        let mut booking = Booking::new(NonZeroUsize::new(2).unwrap());

        assert_eq!(
            booking.apply_milestone_event(0, MilestoneEvent::Start),
            Err(BookingError::BookingNotReadyForMilestone),
        );

        booking.apply_event(BookingEvent::ProviderAccepted).unwrap();
        booking
            .confirm_funding(
                Hold {
                    id: 1,
                    expires_at_hour: 50,
                },
                0,
            )
            .unwrap();
        assert_eq!(
            booking.apply_milestone_event(1, MilestoneEvent::Start),
            Err(BookingError::MilestoneOutOfOrder),
        );

        for (index, expected_state) in [(0, BookingState::InProgress), (1, BookingState::Settling)]
        {
            let delivery = DeliveryId(index as u64 + 1);
            booking
                .apply_milestone_event(index, MilestoneEvent::Start)
                .unwrap();
            booking
                .apply_milestone_event(index, MilestoneEvent::Submit(delivery))
                .unwrap();
            booking
                .apply_milestone_event(index, MilestoneEvent::Approve(delivery))
                .unwrap();
            assert_eq!(booking.state(), expected_state);
            assert_eq!(
                booking.milestones()[index],
                MilestoneState::Approved(delivery)
            );
        }

        let hold_id = booking.request_payout(0).unwrap();
        booking.confirm_payout(hold_id).unwrap();
        assert_eq!(booking.state(), BookingState::Completed);
    }

    #[test]
    // يتحقق من رفض اعتماد الحجز مباشرة من دون المرور بقواعد المرحلة.
    fn final_approval_cannot_bypass_the_milestone_rules() {
        let mut booking = Booking::new(NonZeroUsize::new(1).unwrap());
        booking.apply_event(BookingEvent::ProviderAccepted).unwrap();
        booking
            .confirm_funding(
                Hold {
                    id: 1,
                    expires_at_hour: 50,
                },
                0,
            )
            .unwrap();
        booking
            .apply_milestone_event(0, MilestoneEvent::Start)
            .unwrap();

        assert_eq!(
            booking.apply_event(BookingEvent::FinalMilestoneApproved),
            Err(BookingError::DerivedEventRequired),
        );
        assert_eq!(booking.state(), BookingState::InProgress);
    }
}
