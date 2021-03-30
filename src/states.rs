#![allow(dead_code)]
use crate::resources::WindowSize;
use core::fmt::Debug;
use crossbeam_channel::Receiver;
use legion::{systems::Step, *};
use wgpu::CommandBuffer;

pub enum StateTransition {
    Pop,
    Push(Box<dyn State>),
    Noop,
}
pub trait State: Debug {
    fn on_init(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    );
    // Only called when in foreground
    fn on_foreground_tick(&mut self) -> StateTransition;
    fn on_resize(&mut self, _resources: &Resources, _new_size: &WindowSize) {}
    // Todo: clean up command receivers?
    fn on_destroy(&mut self, world: &mut World, resources: &mut Resources);
    fn background_schedule(&self) -> Schedule;
    fn foreground_schedule(&self) -> Schedule;
}
// Re add type id here if needed later on for downcasting
// or debug logging
#[derive(Debug, Default)]
pub struct StateStack {
    stack: Vec<Box<dyn State>>,
}
// option 1: run each schedule individually drawback, non optimal schedule execution (possible to parellalize more)
// option 2: all passes are resources -> foreground/background can be called many times without constructing more gpu resourcs
// benefits: optimizied scheduling, no extra resource allocation on state transitions,
// option 2 is implemented here
impl StateStack {
    #[must_use]
    pub fn push(
        &mut self,
        mut state: Box<dyn State>,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) -> Vec<Step> {
        // initialize the new state
        info!("Initializing state new state");
        state.on_init(world, resources, command_receivers);
        info!("Pushing state onto stack");
        self.stack.push(state);
        self.calc_schedule_steps()
    }

    #[must_use]
    pub fn push_all(
        &mut self,
        states: impl IntoIterator<Item = Box<dyn State>>,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) -> Vec<Step> {
        states.into_iter().for_each(|mut state| {
            // initialize the new state
            info!("Initializing state: {:?}", state);
            state.on_init(world, resources, command_receivers);
            info!("Pushing state: {:?}", state);
            self.stack.push(state);
        });
        self.calc_schedule_steps()
    }

    #[must_use]
    pub fn pop(&mut self, world: &mut World, resources: &mut Resources) -> Vec<Step> {
        info!("Popping state");
        if let Some(mut current_foreground) = self.stack.pop() {
            info!("Destroying state previous head");
            current_foreground.on_destroy(world, resources);
        }
        self.calc_schedule_steps()
    }

    #[must_use]
    pub fn peek_mut(&mut self) -> Option<&mut Box<dyn State>> {
        self.stack.last_mut()
    }

    pub fn states_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut Box<dyn State + 'static>> {
        self.stack.iter_mut()
    }

    pub fn resize_states(&mut self, new_size: &WindowSize, resources: &Resources) {
        self.stack.iter_mut().rev().for_each(|state| state.on_resize(resources, new_size));
    }

    fn calc_schedule_steps(&self) -> Vec<Step> {
        if self.stack.is_empty() {
            return Vec::new();
        }
        std::iter::once(self.stack.last().unwrap())
            .map(|state| state.foreground_schedule())
            .chain(
                self.stack
                    .iter()
                    .rev()
                    // skip foreground state
                    .skip(1)
                    .map(|state| state.background_schedule()),
            )
            .flat_map(|schedule| schedule.into_vec())
            .collect::<Vec<Step>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paste::paste;
    macro_rules! create_test_state {
        ($state_name:ident) => {
            paste! {
                #[derive(Debug)]
                struct $state_name;

                #[derive(Debug, Default)]
                struct [<$state_name Resources>] {
                    on_init: u32,
                    background_resource: u32,
                    foreground_resource: u32,
                    on_destroy: u32,
                }

                #[system]
                fn [<$state_name foreground>](#[resource] res: &mut [<$state_name Resources>]) {
                    res.foreground_resource += 1;
                }

                #[system]
                fn [<$state_name background>](#[resource] res: &mut [<$state_name Resources>]) {
                    res.background_resource += 1;
                }

                impl State for $state_name {
                    fn on_init(
                        &mut self,
                        _world: &mut World,
                        resources: &mut Resources,
                        _command_receivers: &mut Vec<Receiver<CommandBuffer>>,
                    ) {
                        let mut res = resources.get_mut::<[<$state_name Resources>]>().unwrap();
                        res.on_init += 1;
                    }

                    fn on_foreground_tick(&mut self) -> StateTransition {
                        StateTransition::Noop
                    }

                    fn on_destroy(&mut self, _world: &mut World, resources: &mut Resources) {
                        let mut res = resources.get_mut::<[<$state_name Resources>]>().unwrap();
                        res.on_destroy += 1;
                    }

                    fn background_schedule(&self) -> Schedule {
                        Schedule::builder()
                            .add_system([<$state_name background_system>]())
                            .build()
                    }

                    fn foreground_schedule(&self) -> Schedule {
                        Schedule::builder()
                            .add_system([<$state_name foreground_system>]())
                            .build()
                    }
                }
            }
        };
    }

    create_test_state!(StateA);
    create_test_state!(StateB);
    create_test_state!(StateC);

    #[test]
    fn test_push() {
        let mut world = World::default();
        let mut resources = Resources::default();
        resources.insert(StateAResources::default());

        let mut state_stack = StateStack::default();
        let steps = state_stack.push(
            Box::new(StateA),
            &mut world,
            &mut resources,
            &mut Vec::new(),
        );

        let res = resources.get::<StateAResources>().unwrap();
        assert_eq!(res.on_init, 1);
        drop(res);

        let mut schedule = Schedule::from(steps);
        schedule.execute(&mut world, &mut resources);
        let res = resources.get::<StateAResources>().unwrap();
        assert_eq!(res.on_init, 1);
        assert_eq!(res.foreground_resource, 1);
        assert_eq!(res.background_resource, 0);
        assert_eq!(res.on_destroy, 0);
    }

    #[test]
    fn test_pop() {
        let mut world = World::default();
        let mut resources = Resources::default();
        resources.insert(StateAResources::default());

        let mut state_stack = StateStack::default();
        let steps = state_stack.push(
            Box::new(StateA),
            &mut world,
            &mut resources,
            &mut Vec::new(),
        );

        let mut schedule = Schedule::from(steps);
        schedule.execute(&mut world, &mut resources);

        let _ = state_stack.pop(&mut world, &mut resources);

        let res = resources.get::<StateAResources>().unwrap();
        assert_eq!(res.on_init, 1);
        assert_eq!(res.foreground_resource, 1);
        assert_eq!(res.background_resource, 0);
        assert_eq!(res.on_destroy, 1);
    }

    #[test]
    fn test_transition() {
        let mut world = World::default();
        let mut resources = Resources::default();
        resources.insert(StateAResources::default());
        resources.insert(StateBResources::default());
        resources.insert(StateCResources::default());

        let mut state_stack = StateStack::default();
        let steps = state_stack.push_all(
            vec![
                Box::new(StateC) as Box<dyn State>,
                Box::new(StateB) as Box<dyn State>,
                Box::new(StateA) as Box<dyn State>,
            ],
            &mut world,
            &mut resources,
            &mut Vec::new(),
        );
        let mut schedule = Schedule::from(steps);
        schedule.execute(&mut world, &mut resources);

        let res_a = resources.get::<StateAResources>().unwrap();
        assert_eq!(res_a.on_init, 1);
        assert_eq!(res_a.foreground_resource, 1);
        assert_eq!(res_a.background_resource, 0);
        assert_eq!(res_a.on_destroy, 0);

        let res_b = resources.get::<StateBResources>().unwrap();
        assert_eq!(res_b.on_init, 1);
        assert_eq!(res_b.foreground_resource, 0);
        assert_eq!(res_b.background_resource, 1);
        assert_eq!(res_b.on_destroy, 0);

        let res_c = resources.get::<StateCResources>().unwrap();
        assert_eq!(res_c.on_init, 1);
        assert_eq!(res_c.foreground_resource, 0);
        assert_eq!(res_c.background_resource, 1);
        assert_eq!(res_c.on_destroy, 0);
        drop(res_a);
        drop(res_b);
        drop(res_c);

        let steps = state_stack.pop(&mut world, &mut resources);
        let mut schedule = Schedule::from(steps);
        schedule.execute(&mut world, &mut resources);

        let res_a = resources.get::<StateAResources>().unwrap();
        assert_eq!(res_a.on_init, 1);
        assert_eq!(res_a.foreground_resource, 1);
        assert_eq!(res_a.background_resource, 0);
        assert_eq!(res_a.on_destroy, 1);

        let res_b = resources.get::<StateBResources>().unwrap();
        assert_eq!(res_b.on_init, 1);
        assert_eq!(res_b.foreground_resource, 1);
        assert_eq!(res_b.background_resource, 1);
        assert_eq!(res_b.on_destroy, 0);

        let res_c = resources.get::<StateCResources>().unwrap();
        assert_eq!(res_c.on_init, 1);
        assert_eq!(res_c.foreground_resource, 0);
        assert_eq!(res_c.background_resource, 2);
        assert_eq!(res_c.on_destroy, 0);
        drop(res_a);
        drop(res_b);
        drop(res_c);

        let steps = state_stack.pop(&mut world, &mut resources);
        let mut schedule = Schedule::from(steps);
        schedule.execute(&mut world, &mut resources);
        let res_a = resources.get::<StateAResources>().unwrap();
        assert_eq!(res_a.on_init, 1);
        assert_eq!(res_a.foreground_resource, 1);
        assert_eq!(res_a.background_resource, 0);
        assert_eq!(res_a.on_destroy, 1);

        let res_b = resources.get::<StateBResources>().unwrap();
        assert_eq!(res_b.on_init, 1);
        assert_eq!(res_b.foreground_resource, 1);
        assert_eq!(res_b.background_resource, 1);
        assert_eq!(res_b.on_destroy, 1);

        let res_c = resources.get::<StateCResources>().unwrap();
        assert_eq!(res_c.on_init, 1);
        assert_eq!(res_c.foreground_resource, 1);
        assert_eq!(res_c.background_resource, 2);
        assert_eq!(res_c.on_destroy, 0);
    }
}
