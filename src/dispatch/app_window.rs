use agent_desktop_core::{
    AppError, PlatformAdapter,
    commands::{
        close_app as close_app_command, focus_window as focus_window_command, helpers,
        launch as launch_command, list_apps as list_apps_command,
        list_surfaces as list_surfaces_command, list_windows as list_windows_command,
        maximize as maximize_command, minimize as minimize_command,
        move_window as move_window_command, open_system_surface as open_system_surface_command,
        resize_window as resize_window_command, restore as restore_command,
    },
};
use serde_json::Value;

use crate::cli_args::{
    ListSurfacesArgs,
    system::{
        AppRefArgs, CloseAppArgs, FocusWindowArgs, LaunchArgs, ListAppsArgs, ListWindowsArgs,
        MoveWindowCliArgs, OpenSystemSurfaceArgs, ResizeWindowCliArgs,
    },
};
use crate::dispatch::parse::build_launch_options;
use crate::dispatch::window_cue;
use agent_desktop_core::context::CommandContext;

pub(super) fn launch(
    args: LaunchArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let opening = window_cue::named("Opening", Some(&args.app));
    let shown = window_cue::before(adapter, context, Some(&args.app), None, &opening);
    let value = launch_command::execute(
        launch_command::LaunchArgs {
            app: args.app.clone(),
            options: build_launch_options(&args)?,
        },
        adapter,
    )?;
    if !shown {
        window_cue::after(adapter, context, &value, Some(&args.app), |app| {
            window_cue::named("Opening", app)
        });
    }
    Ok(value)
}

pub(super) fn close_app(
    args: CloseAppArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let closing = window_cue::named("Closing", Some(&args.app));
    window_cue::before(adapter, context, Some(&args.app), None, &closing);
    close_app_command::execute(
        close_app_command::CloseAppArgs {
            app: args.app,
            force: args.force,
        },
        adapter,
    )
}

pub(super) fn list_windows(
    args: ListWindowsArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    list_windows_command::execute(
        list_windows_command::ListWindowsArgs {
            app: args.app,
            timeout_ms: args.timeout_ms,
        },
        adapter,
    )
}

pub(super) fn list_apps(
    args: ListAppsArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    list_apps_command::execute(
        list_apps_command::ListAppsArgs {
            app: args.app,
            timeout_ms: args.timeout_ms,
        },
        adapter,
    )
}

pub(super) fn list_surfaces(
    args: ListSurfacesArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    list_surfaces_command::execute(
        list_surfaces_command::ListSurfacesArgs { app: args.app },
        adapter,
    )
}

pub(super) fn open_system_surface(
    args: OpenSystemSurfaceArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    open_system_surface_command::execute(
        open_system_surface_command::OpenSystemSurfaceArgs {
            surface: args.surface.to_core(),
        },
        adapter,
        context,
    )
}

pub(super) fn focus_window(
    args: FocusWindowArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let app = args.app.clone();
    let switching = window_cue::named("Switching to", app.as_deref());
    let shown = window_cue::before(
        adapter,
        context,
        app.as_deref(),
        args.window_id.as_deref(),
        &switching,
    );
    let value = focus_window_command::execute(
        focus_window_command::FocusWindowArgs {
            window_id: args.window_id,
            app: args.app,
            title: args.title,
        },
        adapter,
    )?;
    if !shown {
        window_cue::after(adapter, context, &value, app.as_deref(), |app| {
            window_cue::named("Switching to", app)
        });
    }
    Ok(value)
}

pub(super) fn resize_window(
    args: ResizeWindowCliArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let app = args.scope.app.clone();
    let cue = window_cue::named("Resizing", app.as_deref());
    let shown = window_cue::before(
        adapter,
        context,
        app.as_deref(),
        args.scope.window_id.as_deref(),
        &cue,
    );
    let value = resize_window_command::execute(
        resize_window_command::ResizeWindowArgs {
            app: args.scope.app,
            window_id: args.scope.window_id,
            width: args.width,
            height: args.height,
        },
        adapter,
    )?;
    if !shown {
        window_cue::after(adapter, context, &value, app.as_deref(), |app| {
            window_cue::named("Resizing", app)
        });
    }
    Ok(value)
}

pub(super) fn move_window(
    args: MoveWindowCliArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let app = args.scope.app.clone();
    let cue = window_cue::named("Moving", app.as_deref());
    let shown = window_cue::before(
        adapter,
        context,
        app.as_deref(),
        args.scope.window_id.as_deref(),
        &cue,
    );
    let value = move_window_command::execute(
        move_window_command::MoveWindowArgs {
            app: args.scope.app,
            window_id: args.scope.window_id,
            x: args.x,
            y: args.y,
        },
        adapter,
    )?;
    if !shown {
        window_cue::after(adapter, context, &value, app.as_deref(), |app| {
            window_cue::named("Moving", app)
        });
    }
    Ok(value)
}

pub(super) fn minimize(
    args: AppRefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let scope = &args.scope;
    let minimizing = window_cue::named("Minimizing", scope.app.as_deref());
    window_cue::before(
        adapter,
        context,
        scope.app.as_deref(),
        scope.window_id.as_deref(),
        &minimizing,
    );
    minimize_command::execute(
        helpers::AppArgs {
            app: args.scope.app,
            window_id: args.scope.window_id,
        },
        adapter,
    )
}

pub(super) fn maximize(
    args: AppRefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let app = args.scope.app.clone();
    let cue = window_cue::named("Maximizing", app.as_deref());
    let shown = window_cue::before(
        adapter,
        context,
        app.as_deref(),
        args.scope.window_id.as_deref(),
        &cue,
    );
    let value = maximize_command::execute(
        helpers::AppArgs {
            app: args.scope.app,
            window_id: args.scope.window_id,
        },
        adapter,
    )?;
    if !shown {
        window_cue::after(adapter, context, &value, app.as_deref(), |app| {
            window_cue::named("Maximizing", app)
        });
    }
    Ok(value)
}

pub(super) fn restore(
    args: AppRefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let app = args.scope.app.clone();
    let cue = window_cue::named("Restoring", app.as_deref());
    let shown = window_cue::before(
        adapter,
        context,
        app.as_deref(),
        args.scope.window_id.as_deref(),
        &cue,
    );
    let value = restore_command::execute(
        helpers::AppArgs {
            app: args.scope.app,
            window_id: args.scope.window_id,
        },
        adapter,
    )?;
    if !shown {
        window_cue::after(adapter, context, &value, app.as_deref(), |app| {
            window_cue::named("Restoring", app)
        });
    }
    Ok(value)
}
