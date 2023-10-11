Here, we use following convention: 
- each model with a corresponding table in the database is defined in a separate file in the models directory
- each materialized view is declared in materialized_views directory
- each udt is declared in udt directory

To define a model with a subset of columns using `partial_model!`, and if we want that model in a 
separate file, we place it in the `{model_name}_partials` directory. For instance, the `user` model 
would be in the `user_partials` directory.




