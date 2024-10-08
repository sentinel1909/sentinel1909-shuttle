mod needs_docker {
    use std::sync::OnceLock;

    use http::StatusCode;
    use permit_client_rs::apis::{
        resource_instances_api::{delete_resource_instance, list_resource_instances},
        users_api::list_users,
    };
    use serial_test::serial;
    use shuttle_backends::client::{
        permit::{Client, Error, Owner, ResponseContent, Team},
        PermissionsDal,
    };
    use shuttle_common::models::{team, user::AccountTier};
    use shuttle_common_tests::permit_pdp::DockerInstance;
    use test_context::{test_context, AsyncTestContext};
    use uuid::Uuid;

    static PDP: OnceLock<DockerInstance> = OnceLock::new();

    #[ctor::dtor]
    fn cleanup() {
        println!("Cleaning up PDP container...");
        if let Some(p) = PDP.get() {
            p.cleanup()
        }
    }

    async fn clear_permit_state(client: &Client) {
        println!("Cleaning up Permit state ahead of test...");

        let users = list_users(
            &client.api,
            &client.proj_id,
            &client.env_id,
            None,
            None,
            None,
            None,
            Some(100),
        )
        .await
        .unwrap();

        for user in users.data {
            client.delete_user(&user.id.to_string()).await.unwrap();
        }

        let resources = list_resource_instances(
            &client.api,
            &client.proj_id,
            &client.env_id,
            None,
            None,
            None,
            None,
            Some(100),
            None,
        )
        .await
        .unwrap();

        for res in resources {
            delete_resource_instance(
                &client.api,
                &client.proj_id,
                &client.env_id,
                &res.id.to_string(),
            )
            .await
            .unwrap();
        }

        println!("Cleaning done.");
    }

    struct Wrap(Client);

    impl AsyncTestContext for Wrap {
        async fn setup() -> Self {
            let api_url = "https://api.eu-central-1.permit.io";
            let api_key = std::env::var("PERMIT_TEST_API_KEY")
                .expect("PERMIT_TEST_API_KEY to be set. You can copy the testing API key from the Testing environment on Permit.io.");

            PDP.get_or_init(|| {
                println!("Starting PDP container...");
                DockerInstance::new(&Uuid::new_v4().to_string(), api_url, &api_key)
            });

            let client = Client::new(
                api_url.to_owned(),
                PDP.get().unwrap().uri.clone(),
                "default".to_owned(),
                "testing".to_owned(),
                api_key,
            );

            clear_permit_state(&client).await;

            Wrap(client)
        }
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_user_flow(Wrap(client): &mut Wrap) {
        let u = "test_user";
        client.new_user(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        // Can also get user by permit id
        let user_by_id = client.get_user(&user.id.to_string()).await.unwrap();

        assert_eq!(user.id, user_by_id.id);

        client.delete_user(u).await.unwrap();
        let res = client.get_user(u).await;

        assert!(matches!(
            res,
            Err(Error::ResponseError(ResponseContent {
                status: StatusCode::NOT_FOUND,
                ..
            }))
        ));
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_tiers_flow(Wrap(client): &mut Wrap) {
        let u = "tier_user";
        client.new_user(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string()
        );

        client.make_pro(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Pro.to_string()
        );

        client.make_basic(u).await.unwrap();
        let user = client.get_user(u).await.unwrap();

        assert_eq!(user.roles.as_ref().unwrap().len(), 1);
        assert_eq!(
            user.roles.as_ref().unwrap()[0].role,
            AccountTier::Basic.to_string()
        );
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_projects(Wrap(client): &mut Wrap) {
        let u1 = "user1";
        let u2 = "user2";
        client.new_user(u1).await.unwrap();
        client.new_user(u2).await.unwrap();

        const SLEEP: u64 = 500;

        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert!(p1.is_empty());

        client.create_project(u1, "proj1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0], "proj1");

        client.create_project(u1, "proj2").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 2);

        client.delete_project("proj1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0], "proj2");

        let p2 = client.get_personal_projects(u2).await.unwrap();

        assert!(p2.is_empty());
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_teams(Wrap(client): &mut Wrap) {
        let u1 = "user-o-1";
        let u2 = "user-o-2";
        client.new_user(u1).await.unwrap();
        client.new_user(u2).await.unwrap();

        const SLEEP: u64 = 500;

        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        let team = Team {
            id: "team_123".to_string(),
            display_name: "Test team".to_string(),
        };

        let err = client.create_team(u1, &team).await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Only Pro users can create teams"
        );

        client.make_pro(u1).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        client.create_team(u1, &team).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let o1 = client.get_teams(u1).await.unwrap();

        assert_eq!(
            o1,
            vec![team::Response {
                id: "team_123".to_string(),
                display_name: "Test team".to_string(),
                is_admin: true,
            }]
        );

        let o = client.get_team(u1, "team_123").await.unwrap();

        assert_eq!(
            o,
            team::Response {
                id: "team_123".to_string(),
                display_name: "Test team".to_string(),
                is_admin: true,
            }
        );

        let err = client
            .create_team(
                u1,
                &Team {
                    id: "team_987".to_string(),
                    display_name: "Second team".to_string(),
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::BAD_REQUEST),
            "User cannot create more than one team"
        );

        client.create_project(u1, "proj-o-1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0], "proj-o-1");

        let own = client.get_project_owner(u1, "proj-o-1").await.unwrap();
        assert_eq!(own, Owner::User(u1.to_string()));

        client
            .transfer_project_to_team(u1, "proj-o-1", "team_123")
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 0);

        let own = client.get_project_owner(u1, "proj-o-1").await.unwrap();
        assert_eq!(own, Owner::Team("team_123".to_string()));

        let err = client.get_team_projects(u2, "team_123").await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "User cannot view projects on a team it does not belong to"
        );

        let ps = client.get_team_projects(u1, "team_123").await.unwrap();
        assert_eq!(ps, vec!["proj-o-1"]);

        client.create_project(u2, "proj-o-2").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p2 = client.get_personal_projects(u2).await.unwrap();

        assert_eq!(p2.len(), 1);
        assert_eq!(p2[0], "proj-o-2");

        let err = client
            .transfer_project_to_team(u2, "proj-o-2", "team_123")
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Cannot transfer to team that user is not admin of"
        );

        let err = client
            .transfer_project_to_team(u1, "proj-o-2", "team_123")
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::NOT_FOUND),
            "Cannot transfer a project that user does not own"
        );

        let err = client.delete_team(u1, "team_123").await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::BAD_REQUEST),
            "Cannot delete team with projects in it"
        );

        let err = client
            .transfer_project_from_team(u2, "proj-o-1", "team_123")
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Cannot transfer from team that user is not admin of"
        );

        client
            .transfer_project_from_team(u1, "proj-o-1", "team_123")
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let p1 = client.get_personal_projects(u1).await.unwrap();

        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0], "proj-o-1");

        let own = client.get_project_owner(u1, "proj-o-1").await.unwrap();
        assert_eq!(own, Owner::User(u1.to_string()));

        let err = client.delete_team(u2, "team_123").await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Cannot delete team that user does not own"
        );

        client.delete_team(u1, "team_123").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let o1 = client.get_teams(u1).await.unwrap();

        assert_eq!(o1, vec![]);
    }

    #[test_context(Wrap)]
    #[tokio::test]
    #[serial]
    async fn test_team_members(Wrap(client): &mut Wrap) {
        let u1 = "user-om-1";
        let u2 = "user-om-2";
        let u3 = "user-om-3";
        client.new_user(u1).await.unwrap();
        client.new_user(u2).await.unwrap();
        client.new_user(u3).await.unwrap();

        const SLEEP: u64 = 500;

        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        client.make_pro(u1).await.unwrap();
        client.make_pro(u3).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        let team = Team {
            id: "team_345".to_string(),
            display_name: "Blazingly fast team".to_string(),
        };

        client.create_team(u1, &team).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        let o1 = client.get_teams(u1).await.unwrap();

        assert_eq!(
            o1,
            vec![team::Response {
                id: "team_345".to_string(),
                display_name: "Blazingly fast team".to_string(),
                is_admin: true,
            }]
        );

        client.create_project(u1, "proj-om-1").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;
        client
            .transfer_project_to_team(u1, "proj-om-1", "team_345")
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        let ps = client.get_team_projects(u1, "team_345").await.unwrap();
        assert_eq!(ps, vec!["proj-om-1"]);

        let o2 = client.get_teams(u2).await.unwrap();
        assert_eq!(o2, vec![]);

        let err = client.get_team_projects(u2, "team_345").await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Cannot view projects of a team that user is not a member of"
        );

        let err = client
            .add_team_member(u1, "team_345", u2)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::BAD_REQUEST),
            "Can only add Pro users to teams"
        );

        client.make_pro(u2).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        client.add_team_member(u1, "team_345", u2).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        let o2 = client.get_teams(u2).await.unwrap();
        assert_eq!(
            o2,
            vec![team::Response {
                id: "team_345".to_string(),
                display_name: "Blazingly fast team".to_string(),
                is_admin: false,
            }]
        );

        let o = client.get_team(u2, "team_345").await.unwrap();

        assert_eq!(
            o,
            team::Response {
                id: "team_345".to_string(),
                display_name: "Blazingly fast team".to_string(),
                is_admin: false,
            }
        );

        let ps2 = client.get_team_projects(u2, "team_345").await.unwrap();
        assert_eq!(ps2, vec!["proj-om-1"]);

        let own = client.get_project_owner(u2, "proj-om-1").await.unwrap();
        assert_eq!(own, Owner::Team("team_345".to_string()));

        let err = client
            .add_team_member(u2, "team_345", u3)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Only team admin can add members"
        );

        let err = client.get_team_members(u3, "team_345").await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Only team members can view members"
        );

        let members = client.get_team_members(u1, "team_345").await.unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&team::MemberResponse {
            id: u1.to_string(),
            role: team::MemberRole::Admin,
        }));
        assert!(members.contains(&team::MemberResponse {
            id: u2.to_string(),
            role: team::MemberRole::Member,
        }));

        let members = client.get_team_members(u2, "team_345").await.unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&team::MemberResponse {
            id: u1.to_string(),
            role: team::MemberRole::Admin,
        }));
        assert!(members.contains(&team::MemberResponse {
            id: u2.to_string(),
            role: team::MemberRole::Member,
        }));

        let err = client
            .remove_team_member(u1, "team_345", u1)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::BAD_REQUEST),
            "User cannot remove themselves from teams"
        );

        let err = client
            .remove_team_member(u2, "team_345", u1)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Only team admin can remove members"
        );

        client.remove_team_member(u1, "team_345", u2).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(SLEEP)).await;

        let o2 = client.get_teams(u2).await.unwrap();
        assert_eq!(o2, vec![]);

        let err = client.get_team_projects(u2, "team_345").await.unwrap_err();
        assert!(
            matches!(err, Error::ResponseError(ResponseContent { status, .. }) if status == StatusCode::FORBIDDEN),
            "Cannot view projects of a team that user is not a member of"
        );

        let members = client.get_team_members(u1, "team_345").await.unwrap();
        assert_eq!(
            members,
            vec![team::MemberResponse {
                id: u1.to_string(),
                role: team::MemberRole::Admin,
            },]
        );
    }
}
