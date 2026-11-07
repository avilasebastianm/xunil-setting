//! Client of `systemd-timedated`, the system service that owns the date, the time and the time zone.
//!
//! Changes go through the service, which asks polkit for permission. See
//! `docs/adr/0008-privileges-through-system-services.md` and `docs/adr/0009-dbus-through-gio.md`.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use gio::glib::variant::{FromVariant, ToVariant};
use gio::glib::{self, Variant};
use gio::{BusType, DBusCallFlags, DBusConnection};

use crate::polkit::AuthorizationFailure;

const BUS_NAME: &str = "org.freedesktop.timedate1";
const OBJECT_PATH: &str = "/org/freedesktop/timedate1";
const INTERFACE: &str = "org.freedesktop.timedate1";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
/// Long enough for the user to read the polkit dialog and type a password.
const CALL_TIMEOUT_MSEC: i32 = 5 * 60 * 1000;
/// Errors that mean nothing provides the service on this system.
const UNAVAILABLE_ERRORS: [&str; 2] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
];

/// The date and time settings, as the service reports them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a separate property of the service"
)]
pub struct TimeDateState {
    /// The time zone, such as `Europe/Madrid`. It is empty when none is set.
    pub timezone: String,
    /// Whether the clock is kept in sync over the network.
    pub ntp: bool,
    /// Whether network time synchronization is available.
    pub can_ntp: bool,
    /// Whether the clock is in sync right now.
    pub ntp_synchronized: bool,
    /// Whether the hardware clock keeps local time instead of UTC.
    pub local_rtc: bool,
    /// The current time, in microseconds since the Unix epoch.
    pub time_usec: u64,
}

impl TimeDateState {
    fn from_properties(properties: &HashMap<String, Variant>) -> Result<Self, TimeDateError> {
        Ok(Self {
            timezone: property(properties, "Timezone")?,
            ntp: property(properties, "NTP")?,
            can_ntp: property(properties, "CanNTP")?,
            ntp_synchronized: property(properties, "NTPSynchronized")?,
            local_rtc: property(properties, "LocalRTC")?,
            time_usec: property(properties, "TimeUSec")?,
        })
    }
}

fn property<T: FromVariant>(
    properties: &HashMap<String, Variant>,
    name: &str,
) -> Result<T, TimeDateError> {
    properties
        .get(name)
        .and_then(Variant::get)
        .ok_or_else(|| TimeDateError::InvalidReply(format!("property {name}")))
}

/// The reason a request to the service failed.
#[derive(Debug)]
pub enum TimeDateError {
    /// The service did not get permission to make the change.
    Authorization(AuthorizationFailure),
    /// Nothing provides the service on this system.
    Unavailable,
    /// The service answered something this client does not understand.
    InvalidReply(String),
    /// The request failed for another reason, such as a time zone the service does not know.
    Failed(glib::Error),
}

impl fmt::Display for TimeDateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authorization(failure) => write!(formatter, "not authorized: {failure:?}"),
            Self::Unavailable => write!(formatter, "the date and time service is not available"),
            Self::InvalidReply(what) => write!(formatter, "unexpected reply: {what}"),
            Self::Failed(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TimeDateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl From<glib::Error> for TimeDateError {
    fn from(error: glib::Error) -> Self {
        if let Some(name) = gio::DBusError::remote_error(&error) {
            if let Some(failure) = AuthorizationFailure::from_dbus_error(&name) {
                return Self::Authorization(failure);
            }
            if UNAVAILABLE_ERRORS.contains(&name.as_str()) {
                return Self::Unavailable;
            }
        }
        Self::Failed(error)
    }
}

/// Talks to the date and time service over D-Bus.
#[derive(Clone, Debug)]
pub struct TimeDateClient {
    connection: DBusConnection,
    destination: Option<&'static str>,
}

impl TimeDateClient {
    /// Connects to the service on the system bus.
    ///
    /// # Errors
    ///
    /// Returns [`TimeDateError`] when the system bus cannot be reached.
    pub async fn system() -> Result<Self, TimeDateError> {
        Ok(Self::new(gio::bus_get_future(BusType::System).await?))
    }

    /// Talks to the service through `connection`, which may also be a direct connection to it.
    #[must_use]
    pub fn new(connection: DBusConnection) -> Self {
        // A direct connection has no bus, so its messages carry no destination.
        let destination = connection.unique_name().map(|_| BUS_NAME);
        Self {
            connection,
            destination,
        }
    }

    /// Reads the current settings.
    ///
    /// # Errors
    ///
    /// Returns [`TimeDateError`] when the service fails or gives an unexpected answer.
    pub async fn state(&self) -> Result<TimeDateState, TimeDateError> {
        let reply = self
            .call(
                PROPERTIES_INTERFACE,
                "GetAll",
                (INTERFACE,).to_variant(),
                DBusCallFlags::NONE,
            )
            .await?;
        let (properties,) = reply
            .get::<(HashMap<String, Variant>,)>()
            .ok_or_else(|| TimeDateError::InvalidReply("GetAll".to_owned()))?;
        TimeDateState::from_properties(&properties)
    }

    /// Lists the time zones the service accepts.
    ///
    /// # Errors
    ///
    /// Returns [`TimeDateError`] when the service fails or gives an unexpected answer.
    pub async fn timezones(&self) -> Result<Vec<String>, TimeDateError> {
        let reply = self
            .call(
                INTERFACE,
                "ListTimezones",
                ().to_variant(),
                DBusCallFlags::NONE,
            )
            .await?;
        let (timezones,) = reply
            .get::<(Vec<String>,)>()
            .ok_or_else(|| TimeDateError::InvalidReply("ListTimezones".to_owned()))?;
        Ok(timezones)
    }

    /// Changes the time zone.
    ///
    /// # Errors
    ///
    /// Returns [`TimeDateError`] when permission is not granted or the service refuses the change.
    pub async fn set_timezone(&self, timezone: &str) -> Result<(), TimeDateError> {
        self.change("SetTimezone", (timezone, true).to_variant())
            .await
    }

    /// Turns network time synchronization on or off.
    ///
    /// # Errors
    ///
    /// Returns [`TimeDateError`] when permission is not granted or the service refuses the change.
    pub async fn set_ntp(&self, enabled: bool) -> Result<(), TimeDateError> {
        self.change("SetNTP", (enabled, true).to_variant()).await
    }

    /// Sets the clock to `time_usec`, in microseconds since the Unix epoch.
    ///
    /// The service refuses this while network time synchronization is on.
    ///
    /// # Errors
    ///
    /// Returns [`TimeDateError`] when permission is not granted or the service refuses the change.
    pub async fn set_time(&self, time_usec: i64) -> Result<(), TimeDateError> {
        self.change("SetTime", (time_usec, false, true).to_variant())
            .await
    }

    /// Calls a method that may need permission. The last argument of each one asks the service to
    /// allow a password dialog, and the message flag lets polkit show it.
    async fn change(&self, method: &str, parameters: Variant) -> Result<(), TimeDateError> {
        self.call(
            INTERFACE,
            method,
            parameters,
            DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
        )
        .await
        .map(drop)
    }

    async fn call(
        &self,
        interface: &str,
        method: &str,
        parameters: Variant,
        flags: DBusCallFlags,
    ) -> Result<Variant, TimeDateError> {
        Ok(self
            .connection
            .call_future(
                self.destination,
                OBJECT_PATH,
                interface,
                method,
                Some(&parameters),
                None,
                flags,
                CALL_TIMEOUT_MSEC,
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::future::Future;
    use std::rc::Rc;

    use gio::{DBusConnectionFlags, DBusMessageFlags, DBusNodeInfo, DBusServer, DBusServerFlags};
    use tempfile::TempDir;

    use super::*;

    const INTERFACE_XML: &str = r#"
        <node>
          <interface name="org.freedesktop.timedate1">
            <method name="SetTimezone">
              <arg type="s" direction="in"/><arg type="b" direction="in"/>
            </method>
            <method name="SetNTP">
              <arg type="b" direction="in"/><arg type="b" direction="in"/>
            </method>
            <method name="SetTime">
              <arg type="x" direction="in"/><arg type="b" direction="in"/>
              <arg type="b" direction="in"/>
            </method>
            <method name="ListTimezones"><arg type="as" direction="out"/></method>
            <property name="Timezone" type="s" access="read"/>
            <property name="NTP" type="b" access="read"/>
            <property name="CanNTP" type="b" access="read"/>
            <property name="NTPSynchronized" type="b" access="read"/>
            <property name="LocalRTC" type="b" access="read"/>
            <property name="TimeUSec" type="t" access="read"/>
          </interface>
        </node>"#;

    /// A method call the fake service received.
    #[derive(Debug, PartialEq)]
    struct Call {
        method: String,
        parameters: Variant,
        interactive: bool,
    }

    /// What the fake service holds and answers.
    struct Fake {
        state: TimeDateState,
        /// When set, every method call fails with this D-Bus error name.
        error: Option<&'static str>,
        calls: Vec<Call>,
    }

    impl Fake {
        fn new() -> Rc<RefCell<Self>> {
            Rc::new(RefCell::new(Self {
                state: TimeDateState {
                    timezone: "Europe/Madrid".to_owned(),
                    ntp: true,
                    can_ntp: true,
                    ntp_synchronized: false,
                    local_rtc: false,
                    time_usec: 1_700_000_000_000_000,
                },
                error: None,
                calls: Vec::new(),
            }))
        }

        fn answer(&mut self, method: &str, parameters: &Variant) -> Option<Variant> {
            match method {
                "ListTimezones" => {
                    return Some((vec!["America/Lima", "Europe/Madrid"],).to_variant());
                }
                "SetTimezone" => {
                    if let Some((timezone, _)) = parameters.get::<(String, bool)>() {
                        self.state.timezone = timezone;
                    }
                }
                "SetNTP" => {
                    if let Some((enabled, _)) = parameters.get::<(bool, bool)>() {
                        self.state.ntp = enabled;
                    }
                }
                "SetTime" => {
                    if let Some((time, _, _)) = parameters.get::<(i64, bool, bool)>() {
                        self.state.time_usec = time.unsigned_abs();
                    }
                }
                _ => {}
            }
            None
        }

        fn property(&self, name: &str) -> Variant {
            let state = &self.state;
            match name {
                "Timezone" => state.timezone.to_variant(),
                "NTP" => state.ntp.to_variant(),
                "CanNTP" => state.can_ntp.to_variant(),
                "NTPSynchronized" => state.ntp_synchronized.to_variant(),
                "LocalRTC" => state.local_rtc.to_variant(),
                _ => state.time_usec.to_variant(),
            }
        }
    }

    fn serve(connection: &DBusConnection, fake: &Rc<RefCell<Fake>>) {
        let node = DBusNodeInfo::for_xml(INTERFACE_XML).expect("interface description");
        let interface = node.lookup_interface(INTERFACE).expect("interface");
        let calls = Rc::clone(fake);
        let properties = Rc::clone(fake);
        connection
            .register_object(OBJECT_PATH, &interface)
            .method_call(move |_, _, _, _, method, parameters, invocation| {
                let mut fake = calls.borrow_mut();
                let flags = invocation.message().flags();
                fake.calls.push(Call {
                    method: method.to_owned(),
                    parameters: parameters.clone(),
                    interactive: flags.contains(DBusMessageFlags::ALLOW_INTERACTIVE_AUTHORIZATION),
                });
                if let Some(error) = fake.error {
                    invocation.return_dbus_error(error, "refused by the fake service");
                } else {
                    let reply = fake.answer(method, &parameters);
                    invocation.return_value(reply.as_ref());
                }
            })
            .property(move |_, _, _, _, name| properties.borrow().property(name))
            .build()
            .expect("registration");
    }

    /// Runs `test` with a client connected straight to a fake service, with no bus in between.
    fn with_fake<F, Fut>(fake: &Rc<RefCell<Fake>>, test: F)
    where
        F: FnOnce(TimeDateClient) -> Fut,
        Fut: Future<Output = ()>,
    {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let socket_dir = TempDir::new().expect("socket directory");
                let address = format!("unix:tmpdir={}", socket_dir.path().display());
                let guid = gio::dbus_generate_guid();
                let server = DBusServer::new_sync(
                    &address,
                    DBusServerFlags::NONE,
                    &guid,
                    None,
                    None::<&gio::Cancellable>,
                )
                .expect("server");
                let shared = Rc::clone(fake);
                // GDBus closes an accepted connection unless something keeps a reference to it.
                let accepted = Rc::new(RefCell::new(Vec::new()));
                let kept = Rc::clone(&accepted);
                server.connect_new_connection(move |_, connection| {
                    serve(connection, &shared);
                    kept.borrow_mut().push(connection.clone());
                    true
                });
                server.start();
                context.block_on(async {
                    let connection = DBusConnection::for_address_future(
                        &server.client_address(),
                        DBusConnectionFlags::AUTHENTICATION_CLIENT,
                        None,
                    )
                    .await
                    .expect("connection");
                    test(TimeDateClient::new(connection)).await;
                });
                server.stop();
            })
            .expect("main context");
    }

    #[test]
    fn reads_the_state() {
        let fake = Fake::new();
        let expected = fake.borrow().state.clone();
        with_fake(&fake, |client| async move {
            assert_eq!(client.state().await.expect("state"), expected);
        });
    }

    #[test]
    fn a_missing_or_mistyped_property_is_an_invalid_reply() {
        let complete: HashMap<String, Variant> = [
            ("Timezone", "Europe/Madrid".to_variant()),
            ("NTP", true.to_variant()),
            ("CanNTP", true.to_variant()),
            ("NTPSynchronized", true.to_variant()),
            ("LocalRTC", false.to_variant()),
            ("TimeUSec", 1_u64.to_variant()),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
        assert!(TimeDateState::from_properties(&complete).is_ok());

        let mut missing = complete.clone();
        missing.remove("NTP");
        let mut mistyped = complete;
        mistyped.insert("TimeUSec".to_owned(), "soon".to_variant());
        for properties in [missing, mistyped] {
            let error = TimeDateState::from_properties(&properties).expect_err("invalid");
            assert!(matches!(error, TimeDateError::InvalidReply(_)), "{error:?}");
        }
    }

    #[test]
    fn lists_the_time_zones() {
        let fake = Fake::new();
        with_fake(&fake, |client| async move {
            assert_eq!(
                client.timezones().await.expect("time zones"),
                ["America/Lima", "Europe/Madrid"]
            );
        });
    }

    #[test]
    fn changes_ask_for_interactive_authorization() {
        let fake = Fake::new();
        with_fake(&fake, |client| async move {
            client
                .set_timezone("America/Lima")
                .await
                .expect("time zone");
            client.set_ntp(false).await.expect("ntp");
            client.set_time(1_800_000_000_000_000).await.expect("time");
        });
        let fake = fake.borrow();
        let expected = [
            ("SetTimezone", ("America/Lima", true).to_variant()),
            ("SetNTP", (false, true).to_variant()),
            (
                "SetTime",
                (1_800_000_000_000_000_i64, false, true).to_variant(),
            ),
        ];
        let calls: Vec<_> = fake.calls.iter().map(|call| &call.method).collect();
        assert_eq!(calls, ["SetTimezone", "SetNTP", "SetTime"]);
        for (call, (method, parameters)) in fake.calls.iter().zip(expected) {
            assert_eq!(call.parameters, parameters, "{method}");
            assert!(call.interactive, "{method}");
        }
        assert_eq!(fake.state.timezone, "America/Lima");
        assert!(!fake.state.ntp);
        assert_eq!(fake.state.time_usec, 1_800_000_000_000_000);
    }

    #[test]
    fn reading_does_not_ask_for_authorization() {
        let fake = Fake::new();
        with_fake(&fake, |client| async move {
            client.timezones().await.expect("time zones");
        });
        assert!(!fake.borrow().calls[0].interactive);
    }

    #[test]
    fn a_refused_change_is_an_authorization_failure() {
        for (name, failure) in [
            (
                "org.freedesktop.PolicyKit1.Error.NotAuthorized",
                AuthorizationFailure::Denied,
            ),
            (
                "org.freedesktop.PolicyKit1.Error.Cancelled",
                AuthorizationFailure::Cancelled,
            ),
            (
                "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired",
                AuthorizationFailure::NoAgent,
            ),
        ] {
            let fake = Fake::new();
            fake.borrow_mut().error = Some(name);
            with_fake(&fake, |client| async move {
                let error = client.set_ntp(false).await.expect_err(name);
                assert!(
                    matches!(error, TimeDateError::Authorization(f) if f == failure),
                    "{name}: {error:?}"
                );
            });
            assert!(fake.borrow().state.ntp, "{name}");
        }
    }

    #[test]
    fn a_missing_service_is_unavailable() {
        for name in UNAVAILABLE_ERRORS {
            let fake = Fake::new();
            fake.borrow_mut().error = Some(name);
            with_fake(&fake, |client| async move {
                let error = client.timezones().await.expect_err(name);
                assert!(
                    matches!(error, TimeDateError::Unavailable),
                    "{name}: {error:?}"
                );
            });
        }
    }

    #[test]
    fn other_errors_keep_the_message_of_the_service() {
        let fake = Fake::new();
        fake.borrow_mut().error = Some("org.freedesktop.DBus.Error.InvalidArgs");
        with_fake(&fake, |client| async move {
            let error = client
                .set_timezone("Nowhere/Nothing")
                .await
                .expect_err("invalid");
            assert!(matches!(error, TimeDateError::Failed(_)), "{error:?}");
            assert!(
                error.to_string().contains("refused by the fake service"),
                "{error}"
            );
            assert!(error.source().is_some());
        });
    }

    #[test]
    fn errors_describe_what_happened() {
        let cases = [
            (
                TimeDateError::Authorization(AuthorizationFailure::NoAgent),
                "not authorized: NoAgent",
            ),
            (
                TimeDateError::Unavailable,
                "the date and time service is not available",
            ),
            (
                TimeDateError::InvalidReply("GetAll".to_owned()),
                "unexpected reply: GetAll",
            ),
        ];
        for (error, text) in cases {
            assert_eq!(error.to_string(), text);
            assert!(error.source().is_none());
        }
    }
}
