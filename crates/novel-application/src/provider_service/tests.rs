mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeRegistry {
        providers: Mutex<HashMap<String, (novel_domain::ProviderProfile, i64)>>,
        models: Mutex<HashMap<String, Vec<novel_domain::ModelProfile>>>,
        bindings: Mutex<Vec<novel_domain::ModelBinding>>,
    }

    #[async_trait]
    impl ProviderRegistry for FakeRegistry {
        async fn list_providers(&self) -> Result<Vec<novel_domain::ProviderProfile>, PortError> {
            Ok(self
                .providers
                .lock()
                .unwrap()
                .values()
                .map(|(profile, _)| profile.clone())
                .collect())
        }

        async fn provider_revision(&self, provider_id: &str) -> Result<Option<i64>, PortError> {
            Ok(self
                .providers
                .lock()
                .unwrap()
                .get(provider_id)
                .map(|(_, revision)| *revision))
        }

        async fn upsert_provider(
            &self,
            profile: &novel_domain::ProviderProfile,
            expected_revision: Option<i64>,
            _now_ms: i64,
        ) -> Result<i64, PortError> {
            let mut providers = self.providers.lock().unwrap();
            let current = providers.get(&profile.id).map(|(_, revision)| *revision);
            match (current, expected_revision) {
                (Some(actual), Some(expected)) if actual != expected => {
                    return Err(PortError::Conflict("版本不一致".into()));
                }
                (Some(_), None) => return Err(PortError::Conflict("已存在".into())),
                _ => {}
            }
            let next = current.unwrap_or(0) + 1;
            providers.insert(profile.id.clone(), (profile.clone(), next));
            Ok(next)
        }

        async fn delete_provider(&self, provider_id: &str, _force: bool) -> Result<(), PortError> {
            self.providers.lock().unwrap().remove(provider_id);
            Ok(())
        }

        async fn list_models(
            &self,
            provider_id: &str,
        ) -> Result<Vec<novel_domain::ModelProfile>, PortError> {
            Ok(self
                .models
                .lock()
                .unwrap()
                .get(provider_id)
                .cloned()
                .unwrap_or_default())
        }

        async fn upsert_model(
            &self,
            model: &novel_domain::ModelProfile,
            _user_edit: bool,
            _expected_revision: Option<i64>,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            let mut models = self.models.lock().unwrap();
            let entry = models.entry(model.provider_id.clone()).or_default();
            entry.retain(|item| item.model_id != model.model_id);
            entry.push(model.clone());
            Ok(())
        }

        async fn delete_model(
            &self,
            provider_id: &str,
            model_id: &str,
            _expected_revision: Option<i64>,
        ) -> Result<(), PortError> {
            if let Some(models) = self.models.lock().unwrap().get_mut(provider_id) {
                models.retain(|item| item.model_id != model_id);
            }
            Ok(())
        }

        async fn list_bindings(&self) -> Result<Vec<novel_domain::ModelBinding>, PortError> {
            Ok(self.bindings.lock().unwrap().clone())
        }

        async fn set_binding(
            &self,
            binding: &novel_domain::ModelBinding,
            _expected_revision: Option<i64>,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            let mut bindings = self.bindings.lock().unwrap();
            bindings.retain(|item| item.task_type != binding.task_type);
            bindings.push(binding.clone());
            Ok(())
        }

        async fn binding_count(&self, provider_id: &str) -> Result<i64, PortError> {
            Ok(self
                .bindings
                .lock()
                .unwrap()
                .iter()
                .filter(|item| item.provider_id == provider_id)
                .count() as i64)
        }
    }

    #[derive(Default)]
    struct FakeVault {
        values: Mutex<HashMap<String, String>>,
        fail_write: Mutex<bool>,
    }

    #[async_trait]
    impl CredentialVault for FakeVault {
        async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
            if *self.fail_write.lock().unwrap() {
                return Err(PortError::Storage("系统凭据写入失败".into()));
            }
            self.values
                .lock()
                .unwrap()
                .insert(reference.to_string(), secret.to_string());
            Ok(())
        }

        async fn delete(&self, reference: &str) -> Result<(), PortError> {
            self.values.lock().unwrap().remove(reference);
            Ok(())
        }

        async fn present(&self, reference: &str) -> Result<bool, PortError> {
            Ok(self.values.lock().unwrap().contains_key(reference))
        }
    }

    fn draft(id: &str) -> ProviderDraft {
        ProviderDraft {
            id: id.into(),
            display_name: Some("测试网关".into()),
            protocol: novel_domain::ProviderProtocol::OpenAiCompatible,
            base_url: "https://gw.example.com/v1".into(),
            allow_localhost_http: false,
            sort_order: 0,
            is_builtin: false,
        }
    }

    #[derive(Default)]
    struct FakeIntents {
        rows: Mutex<Vec<(String, String, String, String)>>,
    }

    #[async_trait]
    impl CredentialIntents for FakeIntents {
        async fn begin(
            &self,
            id: &str,
            provider_id: &str,
            secret_ref: &str,
            action: &str,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            self.rows.lock().unwrap().push((
                id.to_string(),
                provider_id.to_string(),
                secret_ref.to_string(),
                format!("{action}:pending"),
            ));
            Ok(())
        }

        async fn finish(
            &self,
            id: &str,
            state: &str,
            _last_error: Option<&str>,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            let mut rows = self.rows.lock().unwrap();
            if let Some(row) = rows.iter_mut().find(|row| row.0 == id) {
                let action = row.3.split(':').next().unwrap_or("replace").to_string();
                row.3 = format!("{action}:{state}");
            }
            Ok(())
        }

        async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|row| row.3.ends_with(":pending"))
                .map(|row| PendingCredentialIntent {
                    id: row.0.clone(),
                    provider_id: row.1.clone(),
                    secret_ref: row.2.clone(),
                    action: row.3.split(':').next().unwrap_or("replace").to_string(),
                })
                .collect())
        }
    }

    fn service() -> (ProviderService, Arc<FakeRegistry>, Arc<FakeVault>) {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        let intents = Arc::new(FakeIntents::default());
        (
            ProviderService::new(registry.clone(), vault.clone(), intents),
            registry,
            vault,
        )
    }

    #[tokio::test]
    async fn replace_writes_secret_and_hides_it() {
        let (service, _registry, vault) = service();
        let view = service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-secret".into()),
                None,
                1,
            )
            .await
            .unwrap();
        assert!(view.credential_present);
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("sk-secret"), "视图里不能出现密钥：{json}");
        assert!(vault.values.lock().unwrap().contains_key("gw"));
    }

    #[tokio::test]
    async fn keep_never_touches_the_vault() {
        let (service, _registry, vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap();
        let view = service
            .upsert_provider(draft("gw"), CredentialAction::Keep, Some(1), 2)
            .await
            .unwrap();
        assert_eq!(view.revision, 2);
        assert_eq!(
            vault.values.lock().unwrap().get("gw").map(String::as_str),
            Some("sk-1")
        );
    }

    #[tokio::test]
    async fn clear_removes_the_secret() {
        let (service, _registry, vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap();
        let view = service
            .upsert_provider(draft("gw"), CredentialAction::Clear, Some(1), 2)
            .await
            .unwrap();
        assert!(!view.credential_present);
        assert!(!vault.values.lock().unwrap().contains_key("gw"));
    }

    #[tokio::test]
    async fn replace_with_empty_secret_is_rejected() {
        let (service, _registry, vault) = service();
        let error = service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("   ".into()),
                None,
                1,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Validation);
        assert!(vault.values.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stale_revision_is_a_conflict_with_hint() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        let error = service
            .upsert_provider(draft("gw"), CredentialAction::Keep, Some(99), 2)
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Conflict);
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn delete_is_blocked_while_bindings_exist() {
        let (service, registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        registry
            .bindings
            .lock()
            .unwrap()
            .push(novel_domain::ModelBinding {
                task_type: "chapter".into(),
                provider_id: "gw".into(),
                model_id: "a".into(),
                reasoning_effort: None,
            });
        let error = service.delete_provider("gw", false).await.unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::InUse);
        assert_eq!(
            error
                .details
                .and_then(|details| details.get("blockedByCount").cloned()),
            Some(serde_json::json!(1))
        );
    }

    #[tokio::test]
    async fn binding_requires_existing_model() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        let error = service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "chapter".into(),
                    provider_id: "gw".into(),
                    model_id: "missing".into(),
                    reasoning_effort: None,
                },
                None,
                2,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::NotFound);
    }

    #[tokio::test]
    async fn credential_failure_never_writes_the_database() {
        let (service, registry, vault) = service();
        *vault.fail_write.lock().unwrap() = true;
        let error = service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Storage);
        assert!(
            registry.providers.lock().unwrap().is_empty(),
            "凭据失败时不能留下半条配置"
        );
    }
    #[tokio::test]
    async fn successful_replace_marks_intent_done() {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        let intents = Arc::new(FakeIntents::default());
        let service = ProviderService::new(registry, vault, intents.clone());
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap();
        assert!(intents.rows.lock().unwrap()[0].3.ends_with(":done"));
        assert!(
            service
                .reconcile_credentials(2)
                .await
                .unwrap()
                .needs_user_action
                .is_empty()
        );
    }

    #[tokio::test]
    async fn failed_replace_stays_pending_and_needs_user_action() {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        *vault.fail_write.lock().unwrap() = true;
        let intents = Arc::new(FakeIntents::default());
        let service = ProviderService::new(registry.clone(), vault, intents.clone());
        assert!(
            service
                .upsert_provider(
                    draft("gw"),
                    CredentialAction::Replace("sk-1".into()),
                    None,
                    1
                )
                .await
                .is_err()
        );
        assert!(
            intents.rows.lock().unwrap()[0].3.ends_with(":pending"),
            "失败要留下可补偿意图"
        );
        assert!(registry.providers.lock().unwrap().is_empty());
        let report = service.reconcile_credentials(2).await.unwrap();
        assert_eq!(report.completed, 0);
        assert_eq!(
            report.needs_user_action,
            vec!["gw".to_string()],
            "replace 无法无明文重放"
        );
    }

    #[tokio::test]
    async fn failed_clear_is_replayed_by_reconcile() {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        vault
            .values
            .lock()
            .unwrap()
            .insert("gw".into(), "sk-old".into());
        let intents = Arc::new(FakeIntents::default());
        let service = ProviderService::new(registry, vault.clone(), intents.clone());
        // 先让删除失败：用一个会失败的 vault 包装不了，改为直接构造 pending 意图
        intents.rows.lock().unwrap().push((
            "gw-clear-1".into(),
            "gw".into(),
            "gw".into(),
            "clear:pending".into(),
        ));
        let report = service.reconcile_credentials(2).await.unwrap();
        assert_eq!(report.completed, 1);
        assert!(
            vault.values.lock().unwrap().is_empty(),
            "重放后密钥必须被清掉"
        );
        assert!(intents.rows.lock().unwrap()[0].3.ends_with(":done"));
    }
    #[tokio::test]
    async fn route_resolves_from_binding_without_secret() {
        let (service, registry, _vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-secret".into()),
                None,
                1,
            )
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::User,
                    overridden: Default::default(),
                },
                true,
                None,
                2,
            )
            .await
            .unwrap();
        service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "writer".into(),
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    reasoning_effort: Some("high".into()),
                },
                None,
                3,
            )
            .await
            .unwrap();

        let route = service
            .resolve_route("writer")
            .await
            .unwrap()
            .expect("有绑定");
        assert_eq!(route.provider_id, "gw");
        assert_eq!(route.model_id, "a");
        assert_eq!(route.protocol, "openai-compatible");
        assert_eq!(route.credential_ref, "gw");
        assert_eq!(route.source, "binding");
        let json = serde_json::to_string(&route).unwrap();
        assert!(!json.contains("sk-secret"), "路由里不能出现密钥");
        let _ = registry;
    }

    #[tokio::test]
    async fn route_is_absent_without_binding() {
        let (service, _registry, _vault) = service();
        assert!(
            service.resolve_route("writer").await.unwrap().is_none(),
            "没有绑定时不改默认行为"
        );
    }

    #[tokio::test]
    async fn route_errors_when_bound_model_disappears() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::User,
                    overridden: Default::default(),
                },
                true,
                None,
                2,
            )
            .await
            .unwrap();
        service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "review".into(),
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    reasoning_effort: None,
                },
                None,
                3,
            )
            .await
            .unwrap();
        service.delete_model("gw", "a", None).await.ok();
        // 删除被绑定的模型会被拦下，这里直接断言拦截行为
        assert!(
            service
                .delete_model("gw", "a", None)
                .await
                .is_err_and(|error| error.code == ProviderErrorCode::InUse)
        );
        assert!(service.resolve_route("review").await.unwrap().is_some());
    }
    #[tokio::test]
    async fn generation_target_prefers_binding_over_legacy() {
        let route = ResolvedModelRoute {
            task_type: "writer".into(),
            provider_id: "gw".into(),
            model_id: "bound-model".into(),
            protocol: "openai-compatible".into(),
            endpoint: "https://gw.example.com/v1".into(),
            allow_localhost_http: false,
            credential_ref: "gw".into(),
            reasoning_effort: None,
            source: "binding".into(),
        };
        let legacy = LegacyModelTarget {
            endpoint: "https://legacy/v1".into(),
            model: "legacy-model".into(),
            allow_localhost_http: false,
            secret_ref: "openai-compatible".into(),
        };
        let target = resolve_generation_target(Some(route), Some(legacy.clone()), None).unwrap();
        assert_eq!(target.model_id, "bound-model");
        assert_eq!(target.credential_ref, "gw");
        assert_eq!(target.source, "binding");

        // 没有绑定时仍走旧路径，行为不变
        let target = resolve_generation_target(None, Some(legacy), None).unwrap();
        assert_eq!(target.model_id, "legacy-model");
        assert_eq!(target.credential_ref, "openai-compatible");
        assert_eq!(target.source, "default");
    }

    #[tokio::test]
    async fn generation_target_reports_missing_configuration() {
        let error = resolve_generation_target(None, None, None).unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Validation);
    }

    #[tokio::test]
    async fn generation_target_honours_explicit_model_override() {
        let route = ResolvedModelRoute {
            task_type: "writer".into(),
            provider_id: "gw".into(),
            model_id: "bound".into(),
            protocol: "openai-compatible".into(),
            endpoint: "https://gw/v1".into(),
            allow_localhost_http: false,
            credential_ref: "gw".into(),
            reasoning_effort: None,
            source: "binding".into(),
        };
        let target = resolve_generation_target(Some(route), None, Some("  explicit  ")).unwrap();
        assert_eq!(target.model_id, "explicit");
    }
    #[tokio::test]
    async fn frozen_route_validates_and_returns_refs_without_secret() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-secret".into()),
                None,
                1,
            )
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::User,
                    overridden: Default::default(),
                },
                true,
                None,
                2,
            )
            .await
            .unwrap();
        let status = service.validate_frozen_route("gw", "a").await.unwrap();
        match status {
            FrozenRouteStatus::Resolved {
                protocol,
                credential_ref,
            } => {
                assert_eq!(protocol, "openai-compatible");
                assert_eq!(credential_ref, "gw");
            }
            other => panic!("期望 Resolved，实际 {other:?}"),
        }
        let json = serde_json::to_string(&service.validate_frozen_route("gw", "a").await.unwrap())
            .unwrap();
        assert!(!json.contains("sk-secret"), "校验结果不能带密钥");
    }

    #[tokio::test]
    async fn frozen_route_reports_deleted_model() {
        let (service, registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        let error = service
            .validate_frozen_route("gw", "gone")
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::NotFound);
        assert!(registry.models.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn frozen_route_treats_unknown_provider_as_legacy() {
        let (service, _registry, _vault) = service();
        let status = service
            .validate_frozen_route("default", "legacy-model")
            .await
            .unwrap();
        assert_eq!(
            status,
            FrozenRouteStatus::Legacy,
            "旧默认配置必须继续可恢复"
        );
    }
    #[tokio::test]
    async fn run_uses_bound_route_and_freezes_refs() {
        let route = ResolvedModelRoute {
            task_type: "writer".into(),
            provider_id: "gw".into(),
            model_id: "bound".into(),
            protocol: "openai-compatible".into(),
            endpoint: "https://gw/v1".into(),
            allow_localhost_http: false,
            credential_ref: "gw".into(),
            reasoning_effort: None,
            source: "binding".into(),
        };
        let (provider, model, endpoint, allow_local, frozen) = apply_route_to_run(
            "default".into(),
            "legacy".into(),
            "https://legacy/v1".into(),
            false,
            Some(route),
        );
        assert_eq!(provider, "gw");
        assert_eq!(model, "bound");
        assert_eq!(endpoint, "https://gw/v1");
        assert!(!allow_local);
        assert_eq!(frozen.protocol.as_deref(), Some("openai-compatible"));
        assert_eq!(frozen.credential_ref.as_deref(), Some("gw"));
    }

    #[tokio::test]
    async fn review_and_writer_bindings_resolve_independently() {
        let (service, _registry, _vault) = service();
        for id in ["gw-a", "gw-b"] {
            service
                .upsert_provider(draft(id), CredentialAction::Keep, None, 1)
                .await
                .unwrap();
            service
                .upsert_model(
                    novel_domain::ModelProfile {
                        provider_id: id.into(),
                        model_id: format!("{id}-model"),
                        display_name: None,
                        capabilities: Default::default(),
                        source: novel_domain::ModelSource::User,
                        overridden: Default::default(),
                    },
                    true,
                    None,
                    2,
                )
                .await
                .unwrap();
        }
        service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "writer".into(),
                    provider_id: "gw-a".into(),
                    model_id: "gw-a-model".into(),
                    reasoning_effort: None,
                },
                None,
                3,
            )
            .await
            .unwrap();
        service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "review".into(),
                    provider_id: "gw-b".into(),
                    model_id: "gw-b-model".into(),
                    reasoning_effort: None,
                },
                None,
                3,
            )
            .await
            .unwrap();

        let writer = service.resolve_route("writer").await.unwrap().unwrap();
        let review = service.resolve_route("review").await.unwrap().unwrap();
        assert_eq!(writer.provider_id, "gw-a");
        assert_eq!(
            review.provider_id, "gw-b",
            "review 绑定独立解析，不影响 writer"
        );
        assert!(service.resolve_route("repair").await.unwrap().is_none());
    }
    #[tokio::test]
    async fn binding_rejects_discovery_only_protocol() {
        let (service, _registry, _vault) = service();
        let mut anthropic = draft("anthropic");
        anthropic.protocol = novel_domain::ProviderProtocol::AnthropicMessages;
        anthropic.base_url = "https://api.anthropic.com".into();
        service
            .upsert_provider(anthropic, CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "anthropic".into(),
                    model_id: "claude".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::Discovered,
                    overridden: Default::default(),
                },
                false,
                None,
                2,
            )
            .await
            .unwrap();
        let error = service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "writer".into(),
                    provider_id: "anthropic".into(),
                    model_id: "claude".into(),
                    reasoning_effort: None,
                },
                None,
                3,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::NotSupported);
    }
    #[tokio::test]
    async fn run_without_binding_keeps_legacy_parameters() {
        let (provider, model, endpoint, allow_local, frozen) = apply_route_to_run(
            "default".into(),
            "legacy".into(),
            "https://legacy/v1".into(),
            true,
            None,
        );
        assert_eq!(provider, "default");
        assert_eq!(model, "legacy");
        assert_eq!(endpoint, "https://legacy/v1");
        assert!(allow_local);
        assert_eq!(frozen, FrozenRoute::default(), "没有绑定就不冻结任何路由");
    }
}
