use super::*;

impl LogicGridEditor {
    pub(super) fn toggle_simulation_input_bit(&mut self, input: usize, bit: u32) {
        let Some(vm) = &mut self.simulation.vm else {
            return;
        };
        let Some(address) = vm.input_addresses().get(input).copied() else {
            return;
        };
        let Some(value) = self.simulation.input_values.get_mut(input) else {
            return;
        };
        if address >= vm.root_component.memory_size {
            return;
        }
        *value ^= 1_u64 << bit;
        vm.root_memory_mut()[address] |= *value;
    }

    pub(super) fn run_simulation_tick(&mut self) {
        if !self.prepare_simulation() || !self.begin_simulation_tick() {
            return;
        }
        while self.simulation.tick_in_progress {
            self.execute_next_simulation_instruction();
        }
    }

    fn link_called_components(&self, vm: &mut Vm) -> Result<(), String> {
        let mut cache = BTreeMap::<Uuid, Rc<ExecutionComponent>>::new();
        vm.load_components(|called| self.link_compiled(called, &mut cache))
    }

    fn link_compiled(
        &self,
        compiled: Uuid,
        cache: &mut BTreeMap<Uuid, Rc<ExecutionComponent>>,
    ) -> Result<Rc<ExecutionComponent>, String> {
        if let Some(component) = cache.get(&compiled) {
            return Ok(Rc::clone(component));
        }
        let program = self
            .compiled
            .get(&compiled)
            .and_then(|source| source.program.as_ref())
            .ok_or_else(|| format!("component {compiled} has not loaded yet"))?;
        let linked = program
            .program()
            .link_with_source(compiled, |called| self.link_compiled(called, cache))?;
        cache.insert(compiled, Rc::clone(&linked));
        Ok(linked)
    }

    pub(super) fn run_simulation_instruction(&mut self) {
        if !self.prepare_simulation() || !self.begin_simulation_tick() {
            return;
        }
        self.execute_next_simulation_instruction();
    }

    pub(super) fn compile_simulation(&mut self, snapshot: SimulationSnapshot) {
        let previous_input_values = self.simulation.input_values.clone();
        match Vm::from_graph(&self.grid, &snapshot.graph).map_err(|error| format!("{error:?}")) {
            Ok(mut vm) => {
                if let Err(error) = self.link_called_components(&mut vm) {
                    self.simulation = Simulation {
                        snapshot: Some(snapshot),
                        vm: None,
                        error: Some(error),
                        input_values: Vec::new(),
                        steps: 0,
                        instruction_selection: SimulationInstructionSelection::Active,
                        tick_in_progress: false,
                    };
                    return;
                }
                let mut input_values = vec![0; vm.input_addresses().len()];
                for (input, value) in input_values.iter_mut().zip(previous_input_values) {
                    *input = value;
                }
                self.simulation = Simulation {
                    snapshot: Some(snapshot),
                    input_values,
                    vm: Some(vm),
                    error: None,
                    steps: 0,
                    instruction_selection: SimulationInstructionSelection::Active,
                    tick_in_progress: false,
                };
            }
            Err(error) => {
                self.simulation = Simulation {
                    snapshot: Some(snapshot),
                    vm: None,
                    error: Some(error),
                    input_values: Vec::new(),
                    steps: 0,
                    instruction_selection: SimulationInstructionSelection::Active,
                    tick_in_progress: false,
                };
            }
        }
    }

    pub(super) fn prepare_simulation(&mut self) -> bool {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
        }
        self.simulation.vm.is_some()
    }

    pub(super) fn update_simulation_preview(&mut self) {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
            self.run_simulation_tick();
        }
    }

    pub(super) fn begin_simulation_tick(&mut self) -> bool {
        if self.simulation.tick_in_progress {
            return true;
        }
        let Some(vm) = &mut self.simulation.vm else {
            return false;
        };
        vm.begin_tick();
        apply_input_values(vm, &self.simulation.input_values);
        self.simulation.instruction_selection = SimulationInstructionSelection::Active;
        if vm.root_instructions().is_empty() {
            self.simulation.steps += 1;
            return false;
        }
        self.simulation.tick_in_progress = true;
        true
    }

    pub(super) fn execute_next_simulation_instruction(&mut self) {
        let Some(vm) = &mut self.simulation.vm else {
            return;
        };
        vm.execute_instruction();
        self.simulation.instruction_selection = SimulationInstructionSelection::Active;
        if vm.is_tick_complete() {
            self.simulation.steps += 1;
            self.simulation.tick_in_progress = false;
        }
    }

    pub(super) fn simulation_snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            components: self.grid.components().cloned().collect(),
            wires: self.grid.wires().to_vec(),
            graph: self.grid.generate_graph(),
        }
    }
}
pub(super) fn storage_bit_indices(scale: Scale) -> Vec<u32> {
    (0..scale.get() as u32).rev().collect()
}

pub(super) struct SimulationInstructionView<'a> {
    pub(super) name: String,
    pub(super) component: &'a Rc<ExecutionComponent>,
    pub(super) instructions: &'a [Instruction],
    pub(super) next_instruction: Option<usize>,
}

pub(super) fn simulation_instruction_view<'a>(
    vm: &'a Vm,
    selection: &'a SimulationInstructionSelection,
    tick_in_progress: bool,
) -> SimulationInstructionView<'a> {
    match selection {
        SimulationInstructionSelection::ReturnFrame(index) => vm
            .returns
            .get(*index)
            .map(|pc| simulation_pc_instruction_view("Caller", pc, true))
            .unwrap_or_else(|| simulation_pc_instruction_view("Current", &vm.pc, tick_in_progress)),
        SimulationInstructionSelection::Component(component) => SimulationInstructionView {
            name: format!("Target: {}", simulation_component_name(component)),
            component,
            instructions: &component.instructions,
            next_instruction: None,
        },
        SimulationInstructionSelection::Active => {
            simulation_pc_instruction_view("Current", &vm.pc, tick_in_progress)
        }
    }
}

pub(super) fn simulation_pc_instruction_view<'a>(
    name: &str,
    pc: &'a Pc,
    active: bool,
) -> SimulationInstructionView<'a> {
    SimulationInstructionView {
        name: format!("{name}: {}", simulation_component_name(&pc.component)),
        component: &pc.component,
        instructions: pc.instructions(),
        next_instruction: (active && pc.instruction_index < pc.instructions().len())
            .then_some(pc.instruction_index),
    }
}

pub(super) fn simulation_component_name(component: &ExecutionComponent) -> String {
    component
        .source
        .map_or_else(|| "component".to_owned(), |source| source.to_string())
}

pub(super) fn apply_input_values(vm: &mut Vm, values: &[u64]) {
    for (&address, value) in vm.input_addresses().to_vec().iter().zip(values) {
        if address < vm.root_component.memory_size {
            vm.root_memory_mut()[address] |= *value;
        }
    }
}
