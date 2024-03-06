mod api;
mod app;
mod clients;
mod constants;
mod errors;
mod models;
mod utils;

use actix_web::middleware::Logger;
use actix_web::{web, App as ActixWebApp, HttpServer};
use api::*;
use app::App;

#[tokio::main]
async fn main() {
    let app = App::new().await;
    let port = app.port();

    app.init().await;

    // web data
    let app_web_data = web::Data::new(app);
    let db_session_web_data = web::Data::from(app_web_data.db_session.clone());

    HttpServer::new(move || {
        ActixWebApp::new()
            .wrap(Logger::new("%a %r %s %b %{Referer}i %{User-Agent}i %T"))
            .wrap(app_web_data.cors())
            .wrap(app_web_data.session_middleware())
            .app_data(app_web_data.clone())
            .app_data(db_session_web_data.clone())
            .service(web::scope("/ws").service(description_ws))
            .service(
                web::scope("/users")
                    .service(get_user)
                    .service(get_user_by_username)
                    .service(create_user)
                    .service(update_bio)
                    .service(delete_user)
                    .service(login)
                    .service(sync)
                    .service(logout)
                    .service(update_profile_image)
                    .service(delete_profile_image),
            )
            .service(
                web::scope("/nodes")
                    .service(get_nodes)
                    .service(get_node)
                    .service(get_branched_node)
                    .service(create_node)
                    .service(update_node_title)
                    .service(update_node_description)
                    .service(delete_node)
                    .service(get_original_node_description_base64)
                    .service(get_node_description)
                    .service(get_node_description_base64)
                    .service(reorder_nodes)
                    .service(upload_cover_image)
                    .service(delete_cover_image)
                    .service(listen_node_events),
            )
            .service(
                web::scope("/likes")
                    .service(user_likes)
                    .service(get_like_count)
                    .service(create_like)
                    .service(delete_like),
            )
            .service(
                web::scope("/workflows")
                    .service(get_workflow)
                    .service(create_workflow)
                    .service(update_initial_inputs)
                    .service(update_workflow_title)
                    .service(delete_workflow),
            )
            .service(
                web::scope("/flows")
                    .service(create_flow)
                    .service(get_flow_description)
                    .service(update_flow_title)
                    .service(update_flow_description)
                    .service(delete_flow),
            )
            .service(
                web::scope("/flow_steps")
                    .service(create_flow_step)
                    .service(update_flow_step_nodes)
                    .service(update_flow_step_inputs)
                    .service(update_flow_step_outputs)
                    .service(update_flow_step_description)
                    .service(delete_flow_step),
            )
            .service(
                web::scope("input_outputs")
                    .service(create_io)
                    .service(get_io_description)
                    .service(update_io_title)
                    .service(update_io_description)
                    .service(delete_io),
            )
            .service(
                web::scope("contribution_requests")
                    .service(get_contribution_requests)
                    .service(get_contribution_request)
                    .service(create_contribution_request)
                    .service(update_contribution_request_title)
                    .service(update_contribution_request_description)
                    .service(delete_contribution_request)
                    .service(publish)
                    .service(merge_contribution_request),
            )
            .service(
                web::scope("attachments")
                    .service(upload_image)
                    .service(get_presigned_url)
                    .service(create_attachment),
            )
            .service(web::scope("branches").service(restore_node).service(undo_delete_node))
            .service(
                web::scope("comments")
                    .service(get_comments)
                    .service(get_thread_comments)
                    .service(create_comment)
                    .service(update_comment_content)
                    .service(delete_comment),
            )
    })
    .bind(("0.0.0.0", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e));
}
