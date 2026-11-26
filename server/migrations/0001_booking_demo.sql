CREATE TABLE bookings (
    id integer PRIMARY KEY,
    state text NOT NULL CHECK (state IN (
        'Requested', 'AwaitingFunding', 'Confirmed', 'InProgress',
        'Settling', 'Completed', 'Cancelled'
    )),
    hour bigint NOT NULL CHECK (hour >= 0)
);

CREATE TABLE milestones (
    booking_id integer NOT NULL REFERENCES bookings(id),
    position integer NOT NULL CHECK (position >= 0),
    state text NOT NULL CHECK (state IN ('Pending', 'InProgress', 'Submitted', 'Approved')),
    delivery_id bigint CHECK (delivery_id >= 0),
    PRIMARY KEY (booking_id, position),
    CHECK ((delivery_id IS NOT NULL) = (state IN ('Submitted', 'Approved')))
);

CREATE TABLE escrow_holds (
    booking_id integer PRIMARY KEY REFERENCES bookings(id),
    hold_id bigint NOT NULL UNIQUE CHECK (hold_id > 0),
    state text NOT NULL CHECK (state IN ('Held', 'PayoutPending', 'Released')),
    expires_at_hour bigint NOT NULL CHECK (expires_at_hour > 0),
    amount_minor bigint NOT NULL CHECK (amount_minor > 0),
    currency text NOT NULL CHECK (length(currency) = 3)
);

CREATE TABLE escrow_operations (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    hold_id bigint NOT NULL REFERENCES escrow_holds(hold_id),
    kind text NOT NULL CHECK (kind IN ('renewal', 'payout')),
    previous_expiry bigint,
    confirmed_expiry bigint,
    CHECK (
        (kind = 'renewal' AND previous_expiry IS NOT NULL
            AND previous_expiry > 0 AND confirmed_expiry IS NOT NULL
            AND confirmed_expiry > previous_expiry)
        OR (kind = 'payout' AND previous_expiry IS NULL AND confirmed_expiry IS NULL)
    )
);

CREATE UNIQUE INDEX escrow_single_renewal
    ON escrow_operations (hold_id, previous_expiry) WHERE kind = 'renewal';
CREATE UNIQUE INDEX escrow_single_payout
    ON escrow_operations (hold_id) WHERE kind = 'payout';

INSERT INTO bookings (id, state, hour) VALUES (1, 'Requested', 0);
INSERT INTO milestones (booking_id, position, state)
    VALUES (1, 0, 'Pending'), (1, 1, 'Pending');

