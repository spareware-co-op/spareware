// Who may do what. Nothing is allowed unless a rule here says so.
// The rows in the store say who belongs to which hub, project and team; these rules never name a
// person.

// Administrators change the directory.
permit (principal, action == Spare::Action::"directory.edit", resource)
when { principal.admin };

// Someone signing in for the first time binds their login to their own person, and no other.
permit (principal, action == Spare::Action::"login.bind", resource)
when { principal == resource };

// A hub opens for its users and its external users.
permit (principal, action == Spare::Action::"hub.open", resource)
when { principal in resource };

// A project opens only for the people added to it.
permit (principal, action == Spare::Action::"project.open", resource)
when { principal in resource };

// A team's WIP folder opens only for that team.
permit (principal, action == Spare::Action::"folder.open", resource)
when { resource.kind == "wip" && principal in resource.team };

// Shared and Consumed folders open for everyone on the project.
permit (principal, action == Spare::Action::"folder.open", resource)
when { resource.kind != "wip" && principal in resource.team.project };

// A model lives in a WIP folder, so only its team reads it, edits it or links into it.
permit (
  principal,
  action in [Spare::Action::"model.read", Spare::Action::"model.edit", Spare::Action::"model.link"],
  resource
)
when { principal in resource.folder.team };

// A team adds models, publishes to its own Shared folder, and takes packages into its own
// Consumed folder.
permit (
  principal,
  action in [
    Spare::Action::"model.add",
    Spare::Action::"package.publish",
    Spare::Action::"package.consume"
  ],
  resource
)
when { principal in resource };

// A package in a Shared or Consumed folder reads like the folder it sits in.
permit (principal, action == Spare::Action::"package.read", resource)
when { resource.folder.kind != "wip" && principal in resource.folder.team.project };

// A team bridges its own shared package to another hub only if both hubs permit each other.
permit (principal, action == Spare::Action::"package.bridge", resource)
when {
  principal in resource.folder.team &&
  resource.folder.kind == "shared" &&
  context.to_hub in resource.folder.team.project.hub.permits &&
  resource.folder.team.project.hub in context.to_hub.permits
};
