#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneState {
    Pending,
    InProgress,
    Submitted(DeliveryId),
    Approved(DeliveryId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneEvent {
    Start,
    Submit(DeliveryId),
    Approve(DeliveryId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneError {
    InvalidTransition,
    DeliveryMismatch,
}

pub fn transition_milestone(
    state: MilestoneState,
    event: MilestoneEvent,
) -> Result<MilestoneState, MilestoneError> {
    use MilestoneEvent::{Approve, Start, Submit};
    use MilestoneState::{Approved, InProgress, Pending, Submitted};

    match (state, event) {
        (Pending, Start) => Ok(InProgress),
        (InProgress, Submit(delivery)) => Ok(Submitted(delivery)),
        (Submitted(current), Approve(delivery)) if current == delivery => Ok(Approved(delivery)),
        (Submitted(_), Approve(_)) => Err(MilestoneError::DeliveryMismatch),
        _ => Err(MilestoneError::InvalidTransition),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approves_the_submitted_delivery() {
        let delivery = DeliveryId(2);
        let started = transition_milestone(MilestoneState::Pending, MilestoneEvent::Start).unwrap();
        let submitted = transition_milestone(started, MilestoneEvent::Submit(delivery)).unwrap();

        assert_eq!(
            transition_milestone(submitted, MilestoneEvent::Approve(delivery)),
            Ok(MilestoneState::Approved(delivery)),
        );
    }

    #[test]
    fn rejects_approval_before_submission_or_for_an_older_delivery() {
        let older_delivery = DeliveryId(1);
        assert_eq!(
            transition_milestone(
                MilestoneState::InProgress,
                MilestoneEvent::Approve(older_delivery)
            ),
            Err(MilestoneError::InvalidTransition),
        );
        assert_eq!(
            transition_milestone(
                MilestoneState::Submitted(DeliveryId(2)),
                MilestoneEvent::Approve(older_delivery),
            ),
            Err(MilestoneError::DeliveryMismatch),
        );
    }
}
