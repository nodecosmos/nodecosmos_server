For Models we use following convention: 
- each model with a corresponding table in the database is defined in a separate file in the models directory
- each materialized view is declared in materialized_views directory
- each udt is declared in udt directory

To define a model with a subset of columns using `partial_model!`, and if we want that model in a 
separate file, we place it in the `{model_name}` directory.
We define callbacks for a model either in the same file as the model or in a separate
file `callbacks.rs` within model directory.
