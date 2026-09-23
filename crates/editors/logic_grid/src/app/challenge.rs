use super::*;

impl LogicGridEditor {
    #[cfg(test)]
    pub fn active_challenge_id(&self) -> Option<ChallengeId> {
        self.challenge.as_ref().map(|challenge| challenge.id)
    }

    pub(super) fn next_missing_challenge_input(&self) -> Option<(usize, Scale, String)> {
        let challenge = self.challenge.as_ref()?;
        challenge
            .data
            .inputs
            .iter()
            .enumerate()
            .find(|(index, _)| {
                let id = InputId::from_u128(*index as u128);
                self.grid.components().all(|component| {
                    !matches!(component.kind, ComponentKind::Input { id: placed, .. } if placed == id)
                })
            })
            .map(|(index, port)| (index, port.scale, port.label.to_owned()))
    }

    pub(super) fn next_missing_challenge_output(&self) -> Option<(usize, Scale, String)> {
        let challenge = self.challenge.as_ref()?;
        challenge
            .data
            .outputs
            .iter()
            .enumerate()
            .find(|(index, _)| {
                let id = OutputId::from_u128(*index as u128);
                self.grid.components().all(|component| {
                    !matches!(component.kind, ComponentKind::Output { id: placed, .. } if placed == id)
                })
            })
            .map(|(index, port)| (index, port.scale, port.label.to_owned()))
    }

    pub(super) fn active_input_scale(&self) -> Scale {
        self.next_missing_challenge_input()
            .map(|(_, scale, _)| scale)
            .unwrap_or(self.tool.scale)
    }

    pub(super) fn active_output_scale(&self) -> Scale {
        self.next_missing_challenge_output()
            .map(|(_, scale, _)| scale)
            .unwrap_or(self.tool.scale)
    }

    pub(super) fn active_tool_snap(&self) -> Scale {
        match self.tool.kind {
            ToolKind::Input => self.active_input_scale(),
            ToolKind::Output => self.active_output_scale(),
            _ => self.tool.snap(),
        }
    }

    pub(super) fn add_input_at(
        &mut self,
        position: Point,
        rotation: Rotation,
    ) -> Option<ComponentId> {
        if let Some((port, scale, label)) = self.next_missing_challenge_input() {
            let id = self.place(
                position,
                ComponentOrientation::from_rotation(rotation),
                ComponentKind::Input {
                    scale,
                    id: InputId::from_u128(port as u128),
                    label,
                },
            );
            if self.next_missing_challenge_input().is_none() && self.tool.kind == ToolKind::Input {
                self.select_tool();
            }
            Some(id)
        } else if self.challenge.is_none() {
            let scale = self.tool.scale;
            let label = self.io_label.clone();
            Some(self.place(
                position,
                ComponentOrientation::from_rotation(rotation),
                ComponentKind::Input {
                    scale,
                    id: InputId::new(),
                    label,
                },
            ))
        } else {
            None
        }
    }

    pub(super) fn add_output_at(
        &mut self,
        position: Point,
        rotation: Rotation,
    ) -> Option<ComponentId> {
        if let Some((port, scale, label)) = self.next_missing_challenge_output() {
            let id = self.place(
                position,
                ComponentOrientation::from_rotation(rotation),
                ComponentKind::Output {
                    scale,
                    id: OutputId::from_u128(port as u128),
                    label,
                },
            );
            if self.next_missing_challenge_output().is_none() && self.tool.kind == ToolKind::Output
            {
                self.select_tool();
            }
            Some(id)
        } else if self.challenge.is_none() {
            let scale = self.tool.scale;
            let label = self.io_label.clone();
            Some(self.place(
                position,
                ComponentOrientation::from_rotation(rotation),
                ComponentKind::Output {
                    scale,
                    id: OutputId::new(),
                    label,
                },
            ))
        } else {
            None
        }
    }

    pub fn take_challenge_passed(&mut self) -> bool {
        match self.challenge.as_mut() {
            Some(challenge) => std::mem::take(&mut challenge.passed_event),
            None => false,
        }
    }

    pub(super) fn ensure_challenge_test(&mut self) {
        if self.challenge.is_none() {
            return;
        }
        let snapshot = self.simulation_snapshot();
        let up_to_date = self
            .challenge
            .as_ref()
            .is_some_and(|challenge| challenge.test.snapshot.as_ref() == Some(&snapshot));
        if up_to_date {
            return;
        }
        let (input_count, output_count) = match &self.challenge {
            Some(challenge) => (challenge.data.inputs.len(), challenge.data.outputs.len()),
            None => return,
        };
        let test = self.compile_challenge_test(snapshot, input_count, output_count);
        if let Some(challenge) = self.challenge.as_mut() {
            challenge.test = test;
        }
    }

    pub(super) fn compile_challenge_test(
        &mut self,
        snapshot: SimulationSnapshot,
        input_count: usize,
        output_count: usize,
    ) -> ChallengeTest {
        let input_slots = challenge_port_slots(
            self.grid
                .components()
                .filter_map(|component| match component.kind {
                    ComponentKind::Input { id, .. } => Some(id),
                    _ => None,
                }),
            input_count,
            InputId::from_u128,
        );
        let output_slots = challenge_port_slots(
            self.grid
                .components()
                .filter_map(|component| match component.kind {
                    ComponentKind::Output { id, .. } => Some(id),
                    _ => None,
                }),
            output_count,
            OutputId::from_u128,
        );

        self.compile_simulation(snapshot.clone());
        if let Some(error) = self.simulation.error.clone() {
            return ChallengeTest {
                snapshot: Some(snapshot),
                error: Some(error),
                input_slots,
                output_slots,
                actual: vec![Vec::new(); output_count],
                ..ChallengeTest::default()
            };
        }

        ChallengeTest {
            snapshot: Some(snapshot),
            error: None,
            input_slots,
            output_slots,
            next_tick: 0,
            actual: vec![Vec::new(); output_count],
            mismatched: false,
        }
    }

    pub(super) fn challenge_test_reset(&mut self) {
        if let Some(challenge) = self.challenge.as_mut() {
            challenge.test.snapshot = None;
        }
        self.ensure_challenge_test();
    }

    pub(super) fn challenge_test_step(&mut self) {
        self.ensure_challenge_test();
        self.advance_challenge_test_tick();
    }

    pub(super) fn challenge_test_seek(&mut self, tick: usize) {
        self.challenge_test_reset();
        for _ in 0..=tick {
            self.advance_challenge_test_tick();
        }
    }

    pub(super) fn challenge_test_run_all(&mut self) {
        self.ensure_challenge_test();
        loop {
            let more = self.challenge.as_ref().is_some_and(|challenge| {
                let test = &challenge.test;
                test.error.is_none()
                    && self.simulation.vm.is_some()
                    && test.next_tick < challenge.data.ticks
            });
            if !more {
                break;
            }
            self.advance_challenge_test_tick();
        }
    }

    pub(super) fn advance_challenge_test_tick(&mut self) {
        let Some(challenge) = self.challenge.as_ref() else {
            return;
        };
        let Some(vm) = self.simulation.vm.as_mut() else {
            return;
        };
        let test = &challenge.test;
        let data_ticks = challenge.data.ticks;
        if test.error.is_some() || test.next_tick >= data_ticks {
            return;
        }
        let tick = test.next_tick;
        let input_slots = test.input_slots.clone();
        let output_slots = test.output_slots.clone();
        let input_values = challenge
            .data
            .inputs
            .iter()
            .map(|port| {
                let mask = value_mask(port.scale);
                port.values.get(tick).copied().unwrap_or(0) & mask
            })
            .collect::<Vec<_>>();
        let output_expected = challenge
            .data
            .outputs
            .iter()
            .map(|port| {
                let mask = value_mask(port.scale);
                (mask, port.values.get(tick).copied().unwrap_or(0) & mask)
            })
            .collect::<Vec<_>>();

        vm.begin_tick();
        let input_addresses = vm.input_addresses().to_vec();
        for (port, slot) in input_slots.iter().enumerate() {
            let Some(address) = slot.and_then(|slot| input_addresses.get(slot).copied()) else {
                continue;
            };
            if address >= vm.root_component.memory_size {
                continue;
            }
            vm.root_memory_mut()[address] |= input_values[port];
        }
        vm.execute();

        let output_addresses = vm.output_addresses().to_vec();
        let mut actual = Vec::with_capacity(output_slots.len());
        let mut mismatched = false;
        for (port, slot) in output_slots.iter().enumerate() {
            let (mask, expected) = output_expected[port];
            let value = slot
                .and_then(|slot| output_addresses.get(slot).copied())
                .and_then(|address| vm.root_memory().get(address).copied())
                .map(|value| value & mask)
                .unwrap_or(0);
            mismatched |= value != expected;
            actual.push(value);
        }

        let Some(challenge) = self.challenge.as_mut() else {
            return;
        };
        let test = &mut challenge.test;
        test.mismatched |= mismatched;
        for (port, value) in actual.into_iter().enumerate() {
            test.actual[port].push(value);
        }
        test.next_tick += 1;

        let all_ports = test.input_slots.iter().all(Option::is_some)
            && test.output_slots.iter().all(Option::is_some);
        let passed = test.next_tick == data_ticks && !test.mismatched && all_ports;
        if passed {
            challenge.passed_event = true;
        }
    }
}

pub(super) fn challenge_port_slots<T: Ord + Copy>(
    ids: impl Iterator<Item = T>,
    count: usize,
    from_index: impl Fn(u128) -> T,
) -> Vec<Option<usize>> {
    let mut ids: Vec<T> = ids.collect();
    ids.sort();
    ids.dedup();
    (0..count)
        .map(|port| ids.binary_search(&from_index(port as u128)).ok())
        .collect()
}
