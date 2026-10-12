use super::*;

struct TestKey;

impl CacheKey for TestKey {
    fn cache_ptr(&self) -> usize {
        1
    }
}

fn scope_for(target: TargetEnv) -> Rc<ComptimeScopeMap> {
    let mut changes = HashMap::new();
    changes.insert(target_env_symbol(), target);
    ComptimeScopeMap::root(HashMap::new()).sub(changes)
}

#[test]
fn per_comptime_scope_cache_keys_on_target() {
    let mut env = new_env();
    let cache: PerComptimeScopeCache<TestKey, TargetEnv> = PerComptimeScopeCache::new();
    let runs = std::cell::Cell::new(0);
    let get = |env: &mut Env, target: TargetEnv| {
        cache
            .get_or_put(&TestKey, &scope_for(target), env, |_| {
                runs.set(runs.get() + 1);
                Ok(target)
            })
            .unwrap()
    };

    assert_eq!(get(&mut env, TargetEnv::Build), TargetEnv::Build);
    assert_eq!(get(&mut env, TargetEnv::C), TargetEnv::C);
    assert_eq!(get(&mut env, TargetEnv::Build), TargetEnv::Build);
    assert_eq!(get(&mut env, TargetEnv::C), TargetEnv::C);
    assert_eq!(runs.get(), 2);
}
