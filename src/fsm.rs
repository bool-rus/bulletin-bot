
pub trait FillableFrom<T> {
    fn fill_from(&mut self, item: T);
}

pub trait AsPrice<P> {
    fn as_price(&self) -> Option<P>;
}

pub enum State<A> {
    Ready,
    PriceWaitng,
    Filling(A),
    Banned(String),
}

pub enum Signal<T> {
    Create,
    Fill(T),
    Publish,
}

pub enum Response<A> {
    FirstCreate,
    PriceRequest,
    NotPrice,
    FillRequest,
    ContinueFilling,
    WrongMessage,
    CannotPublish,
    Publish(A),
    Banned(String),
}

impl<A> Default for State<A> {
    fn default() -> Self {
        Self::Ready
    }
}

pub trait CanFill<A> {
    fn fill(self, fillable: &mut A);
}

impl<A,T> CanFill<A> for T where A: FillableFrom<T> {
    fn fill(self, fillable: &mut A) {
        fillable.fill_from(self)
    }
}

impl <A> State<A>  {
    pub fn process<T,P>(self, signal: Signal<T>) -> (Self, Response<A>) where T: AsPrice<P> + CanFill<A>, A: From<P> {
        match (self, signal) {
            (State::Banned(cause), _) => (State::Banned(cause.clone()), Response::Banned(cause)),
            (State::Ready, Signal::Fill(_)) => (State::Ready, Response::FirstCreate),
            (State::Ready, Signal::Publish) => (State::Ready, Response::CannotPublish),
            (_, Signal::Create) => (State::PriceWaitng, Response::PriceRequest),
            (State::PriceWaitng, Signal::Fill(msg)) => {
                if let Some(price) = msg.as_price() {
                    (State::Filling(price.into()), Response::FillRequest)
                } else {
                    (State::PriceWaitng, Response::NotPrice)
                }
            }
            (State::PriceWaitng, Signal::Publish) => (State::PriceWaitng, Response::CannotPublish),
            (State::Filling(mut ad), Signal::Fill(msg)) => {
                msg.fill(&mut ad);
                (State::Filling(ad), Response::ContinueFilling)
            }
            (State::Filling(ad), Signal::Publish) => (State::Ready, Response::Publish(ad)),
        }
    }
}