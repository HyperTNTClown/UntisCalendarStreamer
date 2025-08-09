# Untis Calendar Streamer

A custom calendar generator for my school's Untis timetable, built by reverse-engineering a private OAuth2 API to provide a cleaner and more powerful alternative to the official app.

This project was born out of a personal need and evolved into a deep dive into reverse engineering, session management, and API integration. It serves as a real-world case study in solving complex problems when no official documentation is available.

## The Story Behind the Project

At the start of the school year, our grade was given a shared account for the official Untis app. The problem? It showed the schedule for *every* class in the grade, making it cluttered and practically unusable.

Confident there was a better way, I initially built a simple tool using the `untis-rs` library. It connected to Untis's public JSON-RPC API, filtered the data to show only my relevant courses, and generated a clean `.ics` calendar file. This was a huge improvement, and I even shared the calendar stream with my classmates.

**The Crisis:** A few months later, the school announced it was deactivating the shared accounts. Every student would now have a personal account linked through our school's IServ portal, which used a completely different and undocumented authentication system. My tool was dead in the water.

**The Solution:** Rather than abandon the project, I decided to fight for the superior user experience of a clean calendar view. I set up an HTTP proxy to meticulously trace the new login process. This led me down the rabbit hole of the OAuth2 "dance," learning to manage cookies, follow redirects, and extract security tokens directly from HTML to authenticate.

**The Reward:** In the process of this reverse engineering, I discovered that the new API endpoint provided more data than the old one. I could query for "invisible" days far into the future, revealing scheduled class cancellations ahead of time—a feature the official app lacked. I rebuilt the tool from the ground up to support this new flow, resulting in a more resilient and powerful service than before.

## Key Features & Technical Accomplishments

*   **Reverse-Engineered OAuth2 Flow:** Successfully replicated a complex, multi-step OAuth2 login process without any documentation by analyzing raw HTTP traffic.
*   **Stateful Session Management:** Manages a stateful HTTP session across multiple domains (the school's IServ portal and WebUntis), handling cookies and redirects gracefully.
*   **HTML Scraping for Security Tokens:** Parses security credentials (`state`, `nonce`, and CSRF tokens) directly from raw HTML responses to craft subsequent authenticated requests.
*   **Live ICS Calendar Stream:** Serves a dynamically generated `.ics` calendar file over HTTP, which can be subscribed to by any standard calendar application.
*   **Predictive Scheduling:** Leverages the private API to fetch data on future class cancellations, providing insights not available in the official client.
*   **Concurrent Architecture:** Utilizes a multi-threaded Tokio runtime to serve calendar data asynchronously with `hyper` while fetching and updating timetable data in a synchronous background thread.

## A Note on Portability

This tool is a bespoke solution, tightly coupled to the specific IServ-WebUntis integration used by my school, Gymnasium am Markt. It is presented here as a portfolio piece and a technical case study.

Adapting it for another school would be a significant undertaking and would require:

1.  A similar IServ-to-WebUntis OAuth2 login mechanism.
2.  Using browser developer tools or a proxy to find the school-specific values.

Key hard-coded values that would need to be changed include:

*   **URLs:** The IServ OAuth URL in `main.rs`.
*   **OAuth `client_id`:** Hard-coded in `fetch.rs`, though it could be modified to be parsed from the login page HTML.
*   **Cookies:** The `schoolname` and `Tenant` cookies in `main.rs`, which must be copied after selecting the school in a browser.
*   **API Parameters:** The `elementId` and `elementType` in `fetch.rs`'s `generate_params_for_date` function, which are specific to a logged-in user.
*   **Physical Location:** The hard-coded school address in `fetch.rs`.

## Technical Stack

*   **Runtime:** Tokio
*   **HTTP Client:** `reqwest` (with `cookie_store` for session management)
*   **Web Server:** `hyper`
*   **Data Serialization:** `serde`
*   **Calendar Generation:** `ics`
*   **Data Synchronization:** `arc-swap` (for safely updating data used by the web server)

## Getting Started (for Development)

These instructions are for developers interested in seeing the code run.

#### 1. Prerequisites

*   Rust and Cargo toolchain.
*   A valid student account for the target IServ portal.

#### 2. Configuration

Create a `.env` file in the root of the project with your credentials:
    ```env
    USERNAME="your_iserv_username"
    PASSWORD="your_iserv_password"
    ```

#### 3. Build & Run

1.  Clone the repository:
    ```bash
    git clone https://github.com/HyperTNTClown/UntisCalendarStreamer.git
    cd UntisCalendarStreamer
    ```

2.  Run the application:
    ```bash
    cargo run
    ```

    The server will start on `localhost:3022`.

#### 4. Accessing the Calendar

Open a browser or calendar client and subscribe to the following URL format. Separate course names with commas.

`http://localhost:3022/ics?MA1,DE2,EN3`

The parameter filters the timetable to only include subjects with the given shorthands (e.g., `MA1`).

##### Advanced Usage: Aliasing

To further customize the calendar output, you can use the optional `alias` file (if it does not exist, create it in the working directory of the executable). This allows you to change the display name of a course or override its location.

*   **Course Alias:** To rename a course, use the format `shorthand;New Name`.
*   **Location Alias:** To change a course's location, use the format `lShorthand;New Location` (note the `l` prefix).
