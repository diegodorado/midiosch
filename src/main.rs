extern crate midir;

use std::fmt;
use std::time::{Duration, Instant};

use midir::{MidiInput, Ignore, MidiInputPort};

use iced::{
    executor, Align, Application, Checkbox, Column, Command, Container,
    Element, Length, Settings, Subscription, Text, HorizontalAlignment,
};

pub fn main() {
    Midiosch::run(Settings::default())
}

enum Midiosch {
    Loading,
    Loaded(State),
}

type MidiInputPortNames = Vec<String>;

#[derive(Clone,Default)]
struct State {
    last: Vec<iced_native::Event>,
    enabled: bool,
    inputsCount: u32,
    midiPortNames: MidiInputPortNames,
    oscPort: u16,
}

#[derive(Debug, Clone)]
enum Message {
    EventOccurred(iced_native::Event),
    Toggled(bool),
    Loaded(MidiInputPortNames),
    Tick(Instant),
}

impl Application for Midiosch {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Midiosch, Command<Message>) {
        (
            Midiosch::Loading, 
            Command::perform(midiInit(), Message::Loaded),
        )
    }

    fn title(&self) -> String {
        String::from("State - Iced")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match self {
            Midiosch::Loading => {
                match message {
                    Message::Loaded(inputs) => {
                        *self = Midiosch::Loaded( State {
                            enabled: false,
                            midiPortNames: inputs,
                            ..State::default()
                        });
                    }
                    _ => {}
                };

                Command::none()
            }
            Midiosch::Loaded(state) => {
                match message {
                    Message::EventOccurred(event) => {
                        state.last.push(event);
                        if state.last.len() > 5 {
                            let _ = state.last.remove(0);
                        }
                    }
                    Message::Toggled(enabled) => {
                        state.enabled = enabled;
                    }
                    Message::Loaded(_) => {}
                    Message::Tick(_) => {
                        state.inputsCount = state.inputsCount + 1;
                    }
                };

                Command::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self {
            Midiosch::Loading => Subscription::none(),
            Midiosch::Loaded(state) => {
                if state.enabled {
                    iced_native::subscription::events().map(Message::EventOccurred)
                } else {
                    midi::every(Duration::from_millis(1000)).map(Message::Tick)
                }
            }
        }
    }

    fn view(&mut self) -> Element<Message> {
        match self {
            Midiosch::Loading => loading_message(),
            Midiosch::Loaded(state) => {

                let rows = state.last.iter().fold(
                    Column::new().spacing(10),
                    |column, event| {
                        column.push(Text::new(format!("{:?}", event)).size(40))
                    },
                );

                let ports = state.midiPortNames.iter().fold(
                    Column::new().spacing(10),
                    |column, name| {
                        column.push(Text::new(format!("{:?}", name)).size(40))
                    },
                );


                let toggle = Checkbox::new(
                    state.enabled,
                    "Listen to runtime state",
                    Message::Toggled,
                );

                let content = Column::new()
                    .align_items(Align::Center)
                    .spacing(20)
                    .push(rows)
                    .push(ports)
                    .push(Text::new(state.inputsCount.to_string()).size(50))
                    .push(toggle);

                Container::new(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }
        }

    }

}


fn loading_message() -> Element<'static, Message> {
    Container::new(
        Text::new("Loading...")
            .horizontal_alignment(HorizontalAlignment::Center)
            .size(50),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_y()
    .into()
}


async fn midiInit() -> MidiInputPortNames {
    let mut inputs: MidiInputPortNames = Vec::new();
    match MidiInput::new("test input"){
        Ok(midi_in) => {
            for (i, p) in midi_in.ports().iter().enumerate() {
                match midi_in.port_name(p){
                    Ok(name) => {inputs.push(name)}
                    Err(_) => {}
                }
            }
        }
        Err(_) => {}
    }
    inputs
}





mod midi {
    use iced::futures;

    pub fn every(
        duration: std::time::Duration,
    ) -> iced::Subscription<std::time::Instant> {
        iced::Subscription::from_recipe(Every(duration))
    }

    struct Every(std::time::Duration);

    impl<H, I> iced_native::subscription::Recipe<H, I> for Every
    where
        H: std::hash::Hasher,
    {
        type Output = std::time::Instant;

        fn hash(&self, state: &mut H) {
            use std::hash::Hash;

            std::any::TypeId::of::<Self>().hash(state);
            self.0.hash(state);
        }

        fn stream(
            self: Box<Self>,
            _input: futures::stream::BoxStream<'static, I>,
        ) -> futures::stream::BoxStream<'static, Self::Output> {
            use futures::stream::StreamExt;

            async_std::stream::interval(self.0)
                .map(|_| std::time::Instant::now())
                .boxed()
        }
    }
}
