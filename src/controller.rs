use tokio::sync::mpsc::{Receiver};

use crate::client::{KeybaseClient};
use crate::state::ApplicationState;
use crate::types::{ListenerEvent, UiEvent};

pub struct Controller<S, C> {
    client: C,
    state: S,
    ui_receiver: Receiver<UiEvent>,
}

impl<S: ApplicationState, C: KeybaseClient> Controller<S, C>{
    pub fn new(client: C, state: S, receiver: Receiver<UiEvent>) -> Self {
        Controller {
            client,
            state,
            ui_receiver: receiver
        }
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let conversations = self.client.fetch_conversations().await?;
        if !conversations.is_empty() {
            let first_id = conversations[0].id.clone();
            self.state.set_conversations(conversations.into_iter().map(|c| c.into()).collect());
            self.state.set_current_conversation(&first_id);
        }
        Ok(())
    }

    pub async fn process_events(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client_receiver = self.client.get_receiver();
        loop {
            tokio::select! {
                msg = client_receiver.recv() => {
                    if let Some(value) = msg {
                        match value {
                            ListenerEvent::ChatMessage(msg) => {
                                let conversation_id = &msg.msg.conversation_id;
                                self.state.insert_message(conversation_id, msg.msg.clone());
                            }
                        }
                    }
                },
                msg = self.ui_receiver.recv() => {
                    if let Some(value) = msg {
                        match value {
                            UiEvent::SendMessage(msg) => {
                                if let Some(convo) = self.state.get_current_conversation() {
                                    let channel = &convo.data.channel;
                                    self.client.send_message(channel, msg).await?;
                                }
                            },
                            UiEvent::SwitchConversation(conversation_id) => {
                                switch_conversation(&mut self.client, &mut self.state, conversation_id).await?;
                            }
                        }
                    }
                },
            }
        }
    }
}

async fn switch_conversation<S: ApplicationState, C: KeybaseClient>(client: &mut C, state: &mut S, conversation_id: String) -> Result<(), Box<dyn std::error::Error>>{
    let (convo_id, should_fetch) = {
        if let Some(mut convo) = state.get_conversation_mut(&conversation_id){
            if !convo.fetched {
                convo.fetched = true;
                (Some(convo.id.clone()), true)
            } else {
                (Some(convo.id.clone()), false)
            }
        } else {
            (None, false)
        }
    };

    if should_fetch {
        let id = &convo_id.unwrap();
        let convo = state.get_conversation(id).unwrap();
        let messages = client.fetch_messages(&convo.data, 20).await?;
                
        state.get_conversation_mut(id).unwrap().insert_messages(messages);
    }

    state.set_current_conversation(&conversation_id);
    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::client::MockKeybaseClient;
    use crate::state::ApplicationStateInner;
    use crate::conversation;
    use crate::types::*;

    #[tokio::test]
    async fn init() {
        let (_, r) = tokio::sync::mpsc::channel::<UiEvent>(32);
        let mut client = MockKeybaseClient::new();
        client.expect_fetch_conversations()
            .times(1)
            .return_once(|| Ok(vec![]));

        let state = ApplicationStateInner::default();

        let mut controller = Controller::new(client, state, r);
        controller.init().await.unwrap();
    }

    #[tokio::test]
    async fn switch_conversation() {
        let (mut s, r) = tokio::sync::mpsc::channel::<UiEvent>(32);
        let (_, c_recv) = tokio::sync::mpsc::channel::<ListenerEvent>(32);
        let mut client = MockKeybaseClient::new();
        let convo = conversation!("test1");
        let convo2 = conversation!("test2");
        let c1 = convo.clone();
        let c2 = convo2.clone();

        client.expect_get_receiver()
            .times(1)
            .return_once(move || c_recv);

        client.expect_fetch_conversations()
            .times(1)
            .return_once(move || Ok(vec![c1, c2]));

        client.expect_fetch_messages()
            .withf(move |c: &KeybaseConversation, _| c.id == "test1")
            .times(1)
            .return_once(|_, _| Ok(vec![]));

        let state = ApplicationStateInner::default();

        let mut controller = Controller::new(client, state, r);

        controller.init().await.unwrap();

        tokio::spawn(async move {
            s.send(UiEvent::SwitchConversation("test1".to_string())).await.ok();
        });

        tokio::select! {
            _ = controller.process_events() => {},
            _ = tokio::time::delay_for(tokio::time::Duration::from_millis(10)) => {}
        }
    }
}
