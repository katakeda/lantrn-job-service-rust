use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use urlencoding::encode;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    email: String,
    facility_id: i16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facility {
    id: i16,
    facility_id: String,
}

#[derive(Deserialize)]
pub struct GetSubscriptionsResponse {
    data: Vec<Subscription>,
}

#[derive(Deserialize)]
pub struct GetFacilitiesResponse {
    data: Vec<Facility>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SendEmailPayload<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    text_body: &'a str,
    html_body: &'a str,
    message_stream: &'a str,
}

#[derive(Deserialize)]
pub struct GetAvailabilitiesResponse {
    campsites: HashMap<String, Campsite>,
}

#[derive(Deserialize)]
pub struct Campsite {
    availabilities: HashMap<String, String>,
}

#[tokio::main]
async fn main() {
    match get_subscriptions().await {
        Ok(subscriptions) => {
            let m1 = chrono::Utc::now().month();
            let m2 = format!("{:02}", (m1 + 1) % 12);
            let m3 = format!("{:02}", (m1 + 2) % 12);
            let m1 = format!("{:02}", m1);
            let mut facility_id_map: HashMap<String, HashSet<String>> = HashMap::new();
            let facility_ids_str = subscriptions
                .iter()
                .map(|s| s.facility_id.to_string())
                .collect::<Vec<String>>()
                .join(",");
            let facilities = get_facilities(&facility_ids_str)
                .await
                .expect("Failed to fetch facilities");

            for facility in facilities.iter() {
                let facility_id = facility.facility_id.to_string();
                let internal_id = facility.id.to_string();
                populate_map(
                    &mut facility_id_map,
                    facility_id.clone(),
                    internal_id.clone(),
                    m1.to_string(),
                )
                .await;
                populate_map(
                    &mut facility_id_map,
                    facility_id.clone(),
                    internal_id.clone(),
                    m2.to_string(),
                )
                .await;
                populate_map(
                    &mut facility_id_map,
                    facility_id.clone(),
                    internal_id.clone(),
                    m3.to_string(),
                )
                .await;
            }
            for subscription in subscriptions.iter() {
                if let Some(months) = facility_id_map.get(&subscription.facility_id.to_string()) {
                    match send_email(subscription, months).await {
                        Ok(_) => println!("Success: {}", subscription.email),
                        Err(e) => println!("Error for {}: {}", subscription.email, e),
                    }
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

pub async fn get_subscriptions() -> Result<Vec<Subscription>, reqwest::Error> {
    let response = reqwest::Client::new()
        .get(&format!(
            "{}/subscriptions?status=confirmed",
            std::env::var("BACKEND_API_ENDPOINT").expect("Failed to parse BACKEND_API_ENDPOINT")
        ))
        .send()
        .await
        .expect("Failed to execute request.")
        .json::<GetSubscriptionsResponse>()
        .await;
    match response {
        Ok(response) => Ok(response.data),
        Err(e) => Err(e),
    }
}

pub async fn get_facilities(facility_ids: &str) -> Result<Vec<Facility>, reqwest::Error> {
    let response = reqwest::Client::new()
        .get(&format!(
            "{}/facilities?ids={}",
            std::env::var("BACKEND_API_ENDPOINT").expect("Failed to parse BACKEND_API_ENDPOINT"),
            facility_ids
        ))
        .send()
        .await
        .expect("Failed to execute request.")
        .json::<GetFacilitiesResponse>()
        .await;
    match response {
        Ok(response) => Ok(response.data),
        Err(e) => Err(e),
    }
}

pub async fn get_availabilities(
    facility_id: &str,
    month: &str,
) -> Result<Vec<String>, reqwest::Error> {
    let year = chrono::Utc::now().year();
    let start_date = format!("{}-{}-01T00:00:00.000Z", year, month);
    let api_host =
        std::env::var("AVAILABILITY_API_HOST").expect("Failed to parse AVAILABILITY_API_HOST");
    let api_url = format!(
        "{}/api/camps/availability/campground/{}/month?start_date={}",
        api_host,
        facility_id,
        encode(&start_date)
    );
    let response = reqwest::Client::new()
        .get(api_url)
        .send()
        .await
        .expect("Failed to execute request")
        .json::<GetAvailabilitiesResponse>()
        .await;
    match response {
        Ok(response) => {
            let mut availabilities = vec![];
            for site in response.campsites.iter() {
                for availability in site.1.availabilities.iter() {
                    if availability.1 == "Available" {
                        availabilities.push(availability.0.to_string());
                    }
                }
            }
            Ok(availabilities)
        }
        Err(e) => Err(e),
    }
}

pub async fn send_email(
    subscription: &Subscription,
    months: &HashSet<String>,
) -> Result<reqwest::Response, reqwest::Error> {
    let reservation_url =
        std::env::var("RESERVATION_URL").expect("Failed to parse RESERVATION_URL");
    let api_url =
        std::env::var("POSTMARK_API_ENDPOINT").expect("Failed to parse POSTMARK_API_ENDPOINT");
    let api_token =
        std::env::var("POSTMARK_API_TOKEN").expect("Failed to parse POSTMARK_API_TOKEN");
    let months_str = months
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<String>>()
        .join(",");
    let payload = SendEmailPayload {
        from: "info@theboardhop.com",
        to: &subscription.email,
        subject: "Your spot opened up!",
        text_body: &format!(
            "New openings for following months: {}.\nBe first to reserve the spot here: {}/{}",
            months_str, reservation_url, subscription.facility_id
        ),
        html_body: &format!(
            "New openings for following months: {}.\nBe first to reserve the spot here: {}/{}",
            months_str, reservation_url, subscription.facility_id
        ),
        message_stream: "outbound",
    };
    let response = reqwest::Client::new()
        .post(api_url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("X-Postmark-Server-Token", api_token)
        .json(&payload)
        .send()
        .await;
    match response {
        Ok(response) => response.error_for_status(),
        Err(e) => Err(e),
    }
}

pub async fn populate_map(
    map: &mut HashMap<String, HashSet<String>>,
    facility_id: String,
    internal_id: String,
    month: String,
) {
    let availabilities = get_availabilities(&facility_id, &month)
        .await
        .expect(&format!(
            "Failed to get availabilities for {} in {}",
            facility_id, month
        ));
    if availabilities.len() > 0 {
        let new_set = HashSet::new();
        let mut months = map.get(&internal_id).unwrap_or(&new_set).to_owned();
        months.insert(month);
        map.insert(internal_id, months);
    }
}
