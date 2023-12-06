#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct CrisisUpdate {
    id: u64,
    title: String,
    description: String,
    location: String,
    timestamp: u64,
}

// Implementing Storable and BoundedStorable traits for CrisisUpdate
impl Storable for CrisisUpdate {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for CrisisUpdate {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

// Existing thread-local variables and payload structure

thread_local! {
    static CRISIS_MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static CRISIS_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(CRISIS_MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter for crisis updates")
    );

    static CRISIS_STORAGE: RefCell<StableBTreeMap<u64, CrisisUpdate, Memory>> =
        RefCell::new(StableBTreeMap::init(
            CRISIS_MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

// ... (existing thread-local variables and payload structure)

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct CrisisUpdatePayload {
    title: String,
    description: String,
    location: String,
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// 2.7.1 get_crisis_update Function:
#[ic_cdk::query]
fn get_crisis_update(id: u64) -> Result<CrisisUpdate, Error> {
    match _get_crisis_update(&id) {
        Some(update) => Ok(update),
        None => Err(Error::NotFound {
            msg: format!("a crisis update with id={} not found", id),
        }),
    }
}

// 2.7.2 _get_crisis_update Function:
fn _get_crisis_update(id: &u64) -> Option<CrisisUpdate> {
    CRISIS_STORAGE.with(|s| s.borrow().get(id))
}

// Helper method to perform insert for CrisisUpdate
fn do_insert_crisis_update(update: &CrisisUpdate) {
    CRISIS_STORAGE.with(|service| service.borrow_mut().insert(update.id, update.clone()));
}

// 2.7.3 add_crisis_update Function:
#[ic_cdk::update]
fn add_crisis_update(update: CrisisUpdatePayload) -> Option<CrisisUpdate> {
    let id = CRISIS_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter for crisis updates");
    let crisis_update = CrisisUpdate {
        id,
        title: update.title,
        description: update.description,
        location: update.location,
        timestamp: time(),
    };
    do_insert_crisis_update(&crisis_update);
    Some(crisis_update)
}

// 2.7.4 update_crisis_update Function:
#[ic_cdk::update]
fn update_crisis_update(id: u64, payload: CrisisUpdatePayload) -> Result<CrisisUpdate, Error> {
    match CRISIS_STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut update) => {
            update.title = payload.title;
            update.description = payload.description;
            update.location = payload.location;
            update.timestamp = time();
            do_insert_crisis_update(&update);
            Ok(update)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a crisis update with id={}. update not found",
                id
            ),
        }),
    }
}

// 2.7.5 delete_crisis_update Function:
#[ic_cdk::update]
fn delete_crisis_update(id: u64) -> Result<CrisisUpdate, Error> {
    match CRISIS_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(update) => Ok(update),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a crisis update with id={}. update not found.",
                id
            ),
        }),
    }
}

// 2.7.7 list_all_crisis_updates Function:
#[ic_cdk::query]
fn list_all_crisis_updates() -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        service
            .borrow()
            .iter()
            .map(|(_, item)| item.clone())
            .collect()
    })
}

// 2.7.8 get_latest_crisis_update Function:
#[ic_cdk::query]
fn get_latest_crisis_update() -> Option<CrisisUpdate> {
    CRISIS_STORAGE
        .with(|service| {
            let map = service.borrow();
            map.iter().max_by_key(|(_, update)| update.timestamp).map(|(_, update)| update.clone())
        })
}

// 2.7.9 search_crisis_updates_by_location Function:
#[ic_cdk::query]
fn search_crisis_updates_by_location(location: String) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE
        .with(|service| {
            let map = service.borrow();
            map.iter().filter_map(|(_, update)| {
                if update.location == location {
                    Some(update.clone())
                } else {
                    None
                }
            }).collect()
        })
}

// 2.7.10 mark_crisis_update_as_resolved Function:
// #[ic_cdk::update]
// fn mark_crisis_update_as_resolved(id: u64) -> Result<(), Error> {
//     CRISIS_STORAGE
//         .with(|service| {
//             let map = service.borrow();
//             match map.iter().find(|(key, _)| **key == id) {
//                 Some((_, update)) => {
//                     // This line will not work because `update` is not mutable
//                     // update.timestamp = time(); // Update timestamp to mark as resolved
//                     Ok(())
//                 },
//                 None => Err(Error::NotFound {
//                     msg: format!(
//                         "Couldn't mark a crisis update with id={} as resolved. Update not found.",
//                         id
//                     ),
//                 }),
//             }
//         })
// }

// 2.7.10 get_crisis_updates_in_range Function:
#[ic_cdk::query]
fn get_crisis_updates_in_range(start_timestamp: u64, end_timestamp: u64) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, update)| {
                if update.timestamp >= start_timestamp && update.timestamp <= end_timestamp {
                    Some(update.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

// 2.7.11 get_crisis_updates_before Function:
#[ic_cdk::query]
fn get_crisis_updates_before(end_timestamp: u64) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, update)| {
                if update.timestamp < end_timestamp {
                    Some(update.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

// 2.7.12 get_crisis_updates_after Function:
#[ic_cdk::query]
fn get_crisis_updates_after(start_timestamp: u64) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, update)| {
                if update.timestamp > start_timestamp {
                    Some(update.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

// 2.7.16 get_crisis_updates_by_id_range Function:
#[ic_cdk::query]
fn get_crisis_updates_by_id_range(start_id: u64, end_id: u64) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, update)| {
                if update.id >= start_id && update.id <= end_id {
                    Some(update.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

// To generate the Candid interface definitions for our canister
ic_cdk::export_candid!();
