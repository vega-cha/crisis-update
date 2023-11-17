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

impl Storable for CrisisUpdate {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for CrisisUpdate {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static CRISIS_MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct CrisisUpdatePayload {
    title: String,
    description: String,
    location: String,
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    AccessDenied { msg: String },
}

#[ic_cdk::query]
fn get_crisis_update(id: u64) -> Result<CrisisUpdate, Error> {
    match _get_crisis_update(&id) {
        Some(update) => Ok(update),
        None => Err(Error::NotFound {
            msg: format!("a crisis update with id={} not found", id),
        }),
    }
}

fn _get_crisis_update(id: &u64) -> Option<CrisisUpdate> {
    CRISIS_STORAGE.with(|s| s.borrow().get(id))
}

fn do_insert_crisis_update(update: &CrisisUpdate) {
    CRISIS_STORAGE.with(|service| service.borrow_mut().insert(update.id, update.clone()));
}

#[ic_cdk::update]
fn add_crisis_update(update: CrisisUpdatePayload) -> Result<CrisisUpdate, Error> {
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
    Ok(crisis_update)
}

#[ic_cdk::update]
fn update_crisis_update(id: u64, payload: CrisisUpdatePayload) -> Result<CrisisUpdate, Error> {
    if !is_authorized() {
        return Err(Error::AccessDenied {
            msg: "Access denied. User lacks authorization.".to_string(),
        });
    }

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

#[ic_cdk::update]
fn delete_crisis_update(id: u64) -> Result<CrisisUpdate, Error> {
    if !is_authorized() {
        return Err(Error::AccessDenied {
            msg: "Access denied. User lacks authorization.".to_string(),
        });
    }

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

#[ic_cdk::query]
fn get_latest_crisis_update() -> Option<CrisisUpdate> {
    CRISIS_STORAGE
        .with(|service| {
            let map = service.borrow();
            map.iter().max_by_key(|(_, update)| update.timestamp).map(|(_, update)| update.clone())
        })
}

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

fn is_authorized() -> bool {
    // Implement your authorization logic here
    // For example, check if the user has the necessary roles or permissions
    true
}

ic_cdk::export_candid!();
