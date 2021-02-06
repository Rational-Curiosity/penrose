//! Data types for working with X events
use crate::core::{
    bindings::{KeyCode, MouseEvent},
    data_types::{Point, Region},
    xconnection::{Atom, Result, XAtomQuerier, XError, Xid},
};

/// Wrapper around the low level X event types that correspond to request / response data when
/// communicating with the X server itself.
///
/// The variant names and data have developed with the reference xcb implementation in mind but
/// should be applicable for all back ends.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XEvent {
    /// A message has been sent to a particular client
    ClientMessage(ClientMessage),
    /// Client config has changed in some way
    ConfigureNotify(ConfigureEvent),
    /// A client is requesting to be repositioned
    ConfigureRequest(ConfigureEvent),
    /// The mouse pointer has entered a new client window
    Enter(PointerChange),
    /// A part or all of a client has become visible
    Expose(ExposeEvent),
    /// A client window has been closed
    Destroy(Xid),
    /// A grabbed key combination has been entered by the user
    KeyPress(KeyCode),
    /// The mouse pointer has left the current client window
    Leave(PointerChange),
    /// A client window is requesting to be positioned and rendered on the screen.
    MapRequest(Xid, bool),
    /// The mouse has moved or a mouse button has been pressed
    MouseEvent(MouseEvent),
    /// A client property has changed in some way
    PropertyNotify(PropertyEvent),
    /// A randr action has occured (new outputs, resolution change etc)
    RandrNotify,
    /// Focus has moved to a different screen
    ScreenChange,
}

/// Known common client message formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientMessageKind {
    /// Inform a client that it is being closed
    DeleteWindow(Xid),
    /// Request that a client take input focus
    TakeFocus(Xid),
    /// Take ownership of the systray
    ///
    /// Args are the id of the root window and id of the window being used as a systray
    TakeSystrayOwnership(Xid, Xid),
    /// Inform an embedded window that it has gained focus
    XEmbedFocusIn(Xid, Xid),
    /// Inform an embedded window that it has been blocked by a modal dialog
    XEmbedModalityOn(Xid, Xid),
    /// Inform a window that it is being embedded
    XEmbedNotify(Xid, Xid),
    /// Inform an embedded window that it is now active
    XEmbedWindowActivate(Xid, Xid),
}

impl ClientMessageKind {
    /// Build a default [ClientMessage] compatible with X11 / XCB formats.
    ///
    /// Most impls of `X*` traits should be able to use the default data generated by this method,
    /// but if you need to send something else, you can always construct the `ClientMessage`
    /// explicitly.
    pub fn as_message<Q>(&self, s: &Q) -> Result<ClientMessage>
    where
        Q: XAtomQuerier,
    {
        let proto_msg = |id: Xid, atom: Atom| {
            let proto = Atom::WmProtocols.as_ref();
            let data = &[s.atom_id(atom.as_ref())?, 0, 0, 0, 0];
            let mask = ClientEventMask::NoEventMask;
            Ok(ClientMessage::from_data_unchecked(id, mask, proto, data))
        };

        // https://specifications.freedesktop.org/xembed-spec/xembed-spec-latest.html
        let xembed_version = 0;
        let notify = 0;
        let activate = 1;
        let focus_in = 4;
        let modality_on = 10;

        let xembed_msg = |id: Xid, embedder: Xid, kind: u32| {
            let atom = Atom::XEmbed.as_ref();
            let data = &[0, kind, 0, embedder, xembed_version];
            let mask = ClientEventMask::SubstructureNotify;
            Ok(ClientMessage::from_data_unchecked(id, mask, atom, data))
        };

        match self {
            ClientMessageKind::DeleteWindow(id) => proto_msg(*id, Atom::WmDeleteWindow),
            ClientMessageKind::TakeFocus(id) => proto_msg(*id, Atom::WmTakeFocus),

            ClientMessageKind::TakeSystrayOwnership(root_id, systray_id) => {
                let atom = Atom::Manager.as_ref();
                let systray = s.atom_id(Atom::NetSystemTrayS0.as_ref())?;
                let data = &[0, systray, *systray_id, 0, 0];
                let mask = ClientEventMask::SubstructureNotify;
                Ok(ClientMessage::from_data_unchecked(
                    *root_id, mask, atom, data,
                ))
            }

            ClientMessageKind::XEmbedFocusIn(id, other) => xembed_msg(*id, *other, focus_in),
            ClientMessageKind::XEmbedModalityOn(id, other) => xembed_msg(*id, *other, modality_on),
            ClientMessageKind::XEmbedNotify(id, other) => xembed_msg(*id, *other, notify),
            ClientMessageKind::XEmbedWindowActivate(id, other) => xembed_msg(*id, *other, activate),
        }
    }
}

/// Event masks used when sending client events
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientEventMask {
    /// Substructure Notify
    SubstructureNotify,
    /// No Mask: all clients should accept
    NoEventMask,
}

/// A client message that needs to be parsed and handled based on its type
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientMessage {
    /// The ID of the window that sent the message
    pub id: Xid,
    /// The mask to use when sending the event
    pub mask: ClientEventMask,
    /// The data type being set
    pub dtype: String,
    data: Vec<u32>,
}

impl ClientMessage {
    /// The raw data being sent in this message
    pub fn data(&self) -> &[u32] {
        &self.data
    }

    /// Try to build a new ClientMessage. Fails if the data is invalid
    pub fn try_from_data(
        id: Xid,
        mask: ClientEventMask,
        dtype: impl Into<String>,
        data: &[u32],
    ) -> Result<Self> {
        if data.len() != 5 {
            return Err(XError::InvalidClientMessageData(data.len()));
        }

        Ok(Self::from_data_unchecked(id, mask, dtype, data))
    }

    pub(crate) fn from_data_unchecked(
        id: Xid,
        mask: ClientEventMask,
        dtype: impl Into<String>,
        data: &[u32],
    ) -> Self {
        Self {
            id,
            mask,
            dtype: dtype.into(),
            data: data.to_vec(),
        }
    }
}

/// A configure request or notification when a client changes position or size
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigureEvent {
    /// The ID of the window that had a property changed
    pub id: Xid,
    /// The new window size
    pub r: Region,
    /// Is this window the root window?
    pub is_root: bool,
}

/// A notification that a window has become visible
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExposeEvent {
    /// The ID of the window that has become exposed
    pub id: Xid,
    /// The current size and position of the window
    pub r: Region,
    /// How many following expose events are pending
    pub count: usize,
}

/// A notification that the mouse pointer has entered or left a window
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointerChange {
    /// The ID of the window that was entered
    pub id: Xid,
    /// Absolute coordinate of the event
    pub abs: Point,
    /// Coordinate of the event relative to top-left of the window itself
    pub relative: Point,
}

/// A property change on a known client
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PropertyEvent {
    /// The ID of the window that had a property changed
    pub id: Xid,
    /// The property that changed
    pub atom: String,
    /// Is this window the root window?
    pub is_root: bool,
}
