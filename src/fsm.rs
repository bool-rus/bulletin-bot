
type UserId = i64;
type Price = u32;
type FileId = String;

#[derive(Debug, Clone)]
pub struct Ad {
    pub price: Price,
    pub text: String,
    pub photos: Vec<FileId>,
}
impl Ad {
    pub fn new(price: Price) -> Self {
        Self {
            price,
            text: String::new(),
            photos: Vec::new(),
        }
    }
    fn fill<T: IncomeMessage>(&mut self, msg: T) {
        if let Some(text) = msg.text() {
            self.text = text;
        }
        if let Some(photo) = msg.photo_id() {
            self.photos.push(photo);
        }
    }
}

#[derive(Debug, Clone)]
pub enum State {
    Ready,
    PriceWaitng,
    Filling(Ad),
    Banned(String),
    WaitForward,
    WaitCause(UserId),
}

pub enum Signal<T> {
    Unknown,
    Create,
    Message(T),
    Publish,
    Ban,
    Unban,
}

#[derive(Debug)]
pub enum Response {
    FirstCreate,
    PriceRequest,
    NotPrice,
    FillRequest,
    ContinueFilling,
    WrongMessage,
    CannotPublish,
    Publish(Ad),
    Ban(UserId, String),
    Banned(String),
    ForwardMe,
    SendCause,
    Empty,
}

impl Default for State {
    fn default() -> Self {
        Self::Ready
    }
}

pub trait IncomeMessage {
    fn text(&self) -> Option<String>;
    fn price(&self) -> Option<Price> {
        self.text()?.parse().ok()
    }
    fn photo_id(&self) -> Option<String>;
    fn author(&self) -> Option<UserId>;
}

impl IncomeMessage for () {
    fn text(&self) -> Option<String> { None }
    fn photo_id(&self) -> Option<String> { None }
    fn author(&self) -> Option<UserId> { None }
}

impl State {
    pub fn process<T>(self, signal: Signal<T>) -> (Self, Response) where T: IncomeMessage {
        match (self, signal) {
            (_, Signal::Unban) => unimplemented!(),
            (State::Banned(cause), _) => (State::Banned(cause.clone()), Response::Banned(cause)),
            (State::WaitForward, Signal::Message(msg)) => {
                if let Some(user) = msg.author() {
                    (State::WaitCause(user), Response::SendCause)
                } else {
                    (State::WaitForward, Response::WrongMessage)
                }
            },
            (State::WaitCause(user_id), Signal::Message(msg)) =>  if let Some(cause) = msg.text() {
                (State::Ready, Response::Ban(user_id, cause)) 
            } else {
                (State::WaitCause(user_id), Response::Empty)
            }
            (State::Ready, Signal::Message(_)) => (State::Ready, Response::FirstCreate),
            (State::Ready, Signal::Publish) => (State::Ready, Response::CannotPublish),
            (State::PriceWaitng, Signal::Message(msg)) => {
                if let Some(price) = msg.price() {
                    (State::Filling(Ad::new(price)), Response::FillRequest)
                } else {
                    (State::PriceWaitng, Response::NotPrice)
                }
            }
            (State::Filling(mut ad), Signal::Message(msg)) => {
                ad.fill(msg);
                (State::Filling(ad), Response::ContinueFilling)
            }
            (_, Signal::Ban) => (State::WaitForward, Response::ForwardMe),
            (_, Signal::Create) => (State::PriceWaitng, Response::PriceRequest),
            (State::Filling(ad), Signal::Publish) => (State::Ready, Response::Publish(ad)),
            (state, Signal::Publish) => (state, Response::CannotPublish),
            (state, Signal::Unknown) => (state, Response::WrongMessage),
        }
    }
}