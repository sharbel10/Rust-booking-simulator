use std::num::NonZeroUsize;

use booking_core::{
    Booking, BookingEvent, BookingState, DeliveryId, EscrowState, Hold, MilestoneEvent,
};

fn main() {
    let mut booking = Booking::new(NonZeroUsize::new(2).unwrap());
    booking.apply_event(BookingEvent::ProviderAccepted).unwrap();
    let premature_start = booking.apply_milestone_event(0, MilestoneEvent::Start);
    assert!(premature_start.is_err());
    println!("Early start rejected: {premature_start:?}");

    let hold = Hold {
        id: 1,
        expires_at_hour: 50,
    };
    booking.confirm_funding(hold, 0).unwrap();
    approve_delivery(&mut booking, 0, DeliveryId(1));
    assert_eq!(booking.state(), BookingState::InProgress);
    println!(
        "Raw footage approved: {:?}, {:?}",
        booking.state(),
        booking.escrow()
    );

    let renewal = booking.renewal_due(49, 2, 24).unwrap();
    assert_eq!(booking.escrow(), EscrowState::Held(hold));
    booking.confirm_renewal(renewal).unwrap();
    assert!(booking.confirm_renewal(renewal).is_err());
    assert!(booking.renewal_due(49, 2, 24).is_none());
    assert_eq!(
        booking.escrow(),
        EscrowState::Held(Hold {
            id: 1,
            expires_at_hour: 74
        })
    );
    println!("Hour 49: mock provider confirms one renewal, until hour 74.");

    let mut new_booking = Booking::new(NonZeroUsize::MIN);
    new_booking
        .apply_event(BookingEvent::ProviderAccepted)
        .unwrap();
    new_booking
        .confirm_funding(
            Hold {
                id: 2,
                expires_at_hour: 100,
            },
            49,
        )
        .unwrap();
    assert_eq!(new_booking.state(), BookingState::Confirmed);
    println!("Hour 49: another booking confirmed while the founder is away.");

    approve_delivery(&mut booking, 1, DeliveryId(2));
    let hold_id = booking.request_payout(49).unwrap();
    assert_eq!(booking.state(), BookingState::Settling);
    println!("Final video approved: payout pending.");
    booking.confirm_payout(hold_id).unwrap();
    assert_eq!(booking.state(), BookingState::Completed);
    println!("Mock provider confirms payout: {:?}", booking.state());
}

fn approve_delivery(booking: &mut Booking, index: usize, delivery: DeliveryId) {
    booking
        .apply_milestone_event(index, MilestoneEvent::Start)
        .unwrap();
    booking
        .apply_milestone_event(index, MilestoneEvent::Submit(delivery))
        .unwrap();
    booking
        .apply_milestone_event(index, MilestoneEvent::Approve(delivery))
        .unwrap();
}
