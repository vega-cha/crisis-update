#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use validator::Validate;
use ic_cdk::api::{time, caller};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct CrisisUpdate {
    id: u64,
    title: String,
    author: String,
    description: String,
    location: String,
    timestamp: Option<u64>,
    created_at: u64
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

#[derive(candid::CandidType, Serialize, Deserialize, Default, Validate)]
struct CrisisUpdatePayload {
    #[validate(length(min = 1))]
    title: String,
    #[validate(length(min = 10))]
    description: String,
    #[validate(length(min = 2))]
    location: String,
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    InputValidationFailed {msg: String},
    AuthenticationFailed {msg: String}
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
fn add_crisis_update(update: CrisisUpdatePayload) -> Result<CrisisUpdate, Error> {
    // Validates payload
    let check_payload = _check_input(&update);
    // Returns an error if validations failed
    if check_payload.is_err(){
        return Err(check_payload.err().unwrap());
    }
    let id = CRISIS_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter for crisis updates");
    let crisis_update = CrisisUpdate {
        id,
        title: update.title,
        author: caller().to_string(),
        description: update.description,
        location: update.location,
        timestamp: None,
        created_at: time()
    };
    do_insert_crisis_update(&crisis_update);
    Ok(crisis_update)
}

// 2.7.4 update_crisis_update Function:
#[ic_cdk::update]
fn update_crisis_update(id: u64, payload: CrisisUpdatePayload) -> Result<CrisisUpdate, Error> {
    match CRISIS_STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut update) => { 
            // Validates whether caller is the author of the task
            let check_if_author = _check_if_author(&update);
            if check_if_author.is_err() {
                return Err(check_if_author.err().unwrap())
            }
            // Validates payload
            let check_payload = _check_input(&payload);
            // Returns an error if validations failed
            if check_payload.is_err(){
                return Err(check_payload.err().unwrap());
            }            
            update.title = payload.title;
            update.description = payload.description;
            update.location = payload.location;
            update.timestamp = Some(time());
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

    let crisis_update = _get_crisis_update(&id).expect(&format!("couldn't delete a crisis_update with id={}. crisis_update not found.", id));
    // Validates whether caller is the author of the task
    let check_if_author = _check_if_author(&crisis_update);
    if check_if_author.is_err() {
        return Err(check_if_author.err().unwrap())
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

// 2.7.7 list_all_crisis_updates Function:
#[ic_cdk::query]
fn list_all_crisis_updates() -> Result<Vec<CrisisUpdate>, Error> {
    if CRISIS_STORAGE.with(|service| service.borrow().is_empty()) {
        return Err(Error::NotFound { msg: format!("There are currently no crisis update reported in the canister") })
    }

    Ok(    CRISIS_STORAGE.with(|service| {
        service
            .borrow()
            .iter()
            .map(|(_, item)| item.clone())
            .collect()
    }))
}

// 2.7.8 get_latest_crisis_update Function:
#[ic_cdk::query]
fn get_latest_crisis_update() -> Result<CrisisUpdate, Error> {
    let crisis_update = CRISIS_STORAGE
        .with(|service| service.borrow().last_key_value());

    if crisis_update.is_none() {
        return Err(Error::NotFound { msg: format!("There are currently no crisis update reported in the canister") })
    }else{
        Ok(crisis_update.unwrap().1)
    }
}

// 2.7.9 search_crisis_updates_by_location Function:
#[ic_cdk::query]
fn search_crisis_updates_by_location(location: String) -> Result<Vec<CrisisUpdate>, Error> {
    if CRISIS_STORAGE.with(|service| service.borrow().is_empty()) {
        return Err(Error::NotFound { msg: format!("There are currently no crisis update reported in the canister") })
    }
    let filtered_crisis_updates: Vec<CrisisUpdate> = CRISIS_STORAGE
        .with(|service| {
            let map = service.borrow();
            map.iter().filter_map(|(_, update)| {
                if update.location == location {
                    Some(update.clone())
                } else {
                    None
                }
            }).collect()
        });
    if filtered_crisis_updates.len() == 0 {
        return Err(Error::NotFound { msg: format!("There are currently no crisis update reported in the location ={}", location) })
    }
    Ok(filtered_crisis_updates)
}

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

// 2.7.20 get_crisis_updates_by_title Function:
#[ic_cdk::query]
fn get_crisis_updates_by_title(title: String) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, update)| {
                if update.title.contains(&title) {
                    Some(update.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

// 2.7.21 get_crisis_updates_by_description Function:
#[ic_cdk::query]
fn get_crisis_updates_by_description(description: String) -> Vec<CrisisUpdate> {
    CRISIS_STORAGE.with(|service| {
        let map = service.borrow();
        map.iter()
            .filter_map(|(_, update)| {
                if update.description.contains(&description) {
                    Some(update.clone())
                } else {
                    None
                }
            })
            .collect()
    })
}

// Helper function to check the input data of the payload
fn _check_input(payload: &CrisisUpdatePayload) -> Result<(), Error> {
    let check_payload = payload.validate();
    if check_payload.is_err() {
        return Err(Error:: InputValidationFailed{ msg: check_payload.err().unwrap().to_string()})
    }else{
        Ok(())
    }
}

// Helper function to check whether the caller is the author of a crisis_update
fn _check_if_author(crisis_update: &CrisisUpdate) -> Result<(), Error> {
    if crisis_update.author.to_string() != caller().to_string(){
        return Err(Error:: AuthenticationFailed{ msg: format!("Caller={} isn't the author of the crisis_update with id={}", caller(), crisis_update.id) })  
    }else{
        Ok(())
    }
}

// To generate the Candid interface definitions for our canister
ic_cdk::export_candid!();
