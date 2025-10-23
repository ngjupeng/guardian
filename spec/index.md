# Private State Manager Specification

Private state manager is a system that allows a device, or a group of devices, to backup and sync their state securely without trust assumptions about other participants or the server operator.

It consists of 2 main elements:

- State: canonical representation of the state of an entity.
- Delta: valid changes applied to the state.


## Definitions

### State

A state is a data structure that represents the current state of a local account, contract, or any other entity that lives in the local device and has to be kept private and in sync with other devices and valid against some network that asserts its validity.

Example:
```json
{
    "account_id": "1234567890",
    "commitment": "0x1234567890",
    "nonce": 10,
    "assets": [
      {
        "balance": 12000,
        "asset_id": "USDC",
      },
      {
        "balance": 2,
        "asset_id": "ETH",
      }
    ],
}
```

### Delta

A delta is whatever changes you apply to that state in append-only operations. The change on the state is also validated against some network state and acknowledged (signed) by the private state manager.

Example:
```json
{
    "account_id": "1234567890",
    "prev_commitment": "0x1234567890",
    "ops": [
      { 
        "type": "transfer",
        "asset_id": "USDC",
        "amount": 100,
      }
    ],
}
```

### Account ID

Is the unique identifier of an account holding a state, the private state manager can host multiple accounts and route authenticated requests to each.

### Commitment

Is the commitment of the state, it's a hash, nonce, or any other identifier that serves as the unique identifier of the current state of the account. It's used to cerifify that the state is not forked or corrupted. Each new delta includes a prev_commitment field that references the commitment of the base state in which the delta is applied.

## Basic principles

- Both State and Deltas are represented as generic JSON objects and completely agnostic of the underlying data model.
- The state should never be forked or corrupted, each delta is validated against the previous state and (optionally) the network state.
- The state must be protected against **external users**, only the account shareholders should be able to access the state.
- The state must be protected against **internal users**, the state should be modified only applying valid deltas, that (optionally) can be verified against the network.
- The state must be protected against the **Private State Manager server operator**, it will support running the server in a secure enclave, doing TLS termination inside the enclave + encrypted storage.
- The implementation is very extensible in different dimensions:
 - The Network against which the state is validated (Miden, Ethereum, Bitcoin, etc.)
 - The underlying storage (filesystem, database, etc.)
 - The requests authentication (public/private keys, JWT etc.)
 - The acknowledgement (ack) signature scheme (falcon, ed25519, etc.)

## Components

### Ack

Ack acts as the component that generates proofs of stored deltas, ideally clients integrating Private State Manager will require this ack proof in order to perform some network operation, like submitting a transaction.

Ack can be implemented in different ways, but the most practical implementation is to use asymetric cryptography, like Falcon or ECDSA, in the future we might include support to other primitives, like ZK proofs.

```rust
pub trait Ack {
    // Initial implementations will use asymetric
    // cryptography, extensible to multiple schemes.
    pub fn pubkey(&self) -> String;

    // Receives a delta with no acknowledgement and
    // returns it with an acknowledgement in it.
    pub fn ack_delta(&self, delta: &DeltaObject) -> Result<DeltaObject>;
}
```
