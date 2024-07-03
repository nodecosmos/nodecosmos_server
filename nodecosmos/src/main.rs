use actix_web::middleware::Logger;
use actix_web::{web, App as ActixWebApp, HttpServer};

use api::*;
use app::App;

mod api;
mod app;
mod constants;
mod errors;
mod models;
mod resources;
mod tasks;

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            {
                let app = App::new().await;
                let port = app.port();

                app.init().await;
                let app_web_data = web::Data::new(app);

                HttpServer::new(move || {
                    // web data
                    let db_session_web_data = web::Data::from(app_web_data.db_session.clone());

                    ActixWebApp::new()
                        .wrap(Logger::new("%a %r %s %b %{Referer}i %{User-Agent}i %T"))
                        .wrap(app_web_data.cors())
                        .wrap(app_web_data.session_middleware())
                        .app_data(app_web_data.clone())
                        .app_data(db_session_web_data.clone())
                        .service(
                            web::scope("/users")
                                .service(get_user)
                                .service(get_user_by_username)
                                .service(create_user)
                                .service(search_users)
                                .service(confirm_user_email)
                                .service(resend_confirmation_email)
                                .service(reset_password_email)
                                .service(update_password)
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
                                .service(delete_node)
                                .service(reorder_nodes)
                                .service(upload_cover_image)
                                .service(delete_cover_image)
                                .service(listen_node_events)
                                .service(get_node_editors)
                                .service(delete_node_editor),
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
                                .service(get_workflow_branch_commit_data)
                                .service(update_workflow_title),
                        )
                        .service(
                            web::scope("/flows")
                                .service(create_flow)
                                .service(update_flow_title)
                                .service(delete_flow),
                        )
                        .service(
                            web::scope("/flow_steps")
                                .service(create_flow_step)
                                .service(update_flow_step_nodes)
                                .service(update_flow_step_inputs)
                                .service(delete_flow_step),
                        )
                        .service(
                            web::scope("input_outputs")
                                .service(create_io)
                                .service(update_io_title)
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
                        .service(
                            web::scope("branches")
                                .service(show_branch)
                                .service(get_branch_node_id)
                                .service(restore_node)
                                .service(undo_delete_node)
                                .service(restore_io)
                                .service(undo_delete_initial_io)
                                .service(undo_delete_flow_step_io)
                                .service(restore_flow)
                                .service(undo_delete_flow)
                                .service(restore_flow_step)
                                .service(keep_flow_step)
                                .service(undo_delete_flow_step)
                                .service(get_branch_editors),
                        )
                        .service(
                            web::scope("comments")
                                .service(get_threads)
                                .service(get_thread_comments)
                                .service(create_comment)
                                .service(update_comment_content)
                                .service(delete_thread)
                                .service(delete_comment),
                        )
                        .service(
                            web::scope("descriptions")
                                .service(get_description)
                                .service(get_base64_description)
                                .service(get_original_description)
                                .service(save_description),
                        )
                        .service(web::scope("ws").service(description_ws))
                        .service(
                            web::scope("invitations")
                                .service(create_invitation)
                                .service(get_invitations)
                                .service(get_invitation_by_token)
                                .service(confirm_invitation)
                                .service(reject_invitation)
                                .service(delete_invitation),
                        )
                        .service(
                            web::scope("notifications")
                                .service(get_notifications)
                                .service(mark_all_as_read),
                        )
                        .service(web::scope("contacts").service(create_contact_us))
                        .service(web::resource("/health").route(web::get().to(|| async { "OK" })))
                })
                .keep_alive(std::time::Duration::from_secs(60))
                .shutdown_timeout(30)
                .max_connections(50_000)
                .bind(("0.0.0.0", port))
                .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
                .run()
                .await
                .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e))
            }
        });
}
