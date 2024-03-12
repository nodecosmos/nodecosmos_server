### Models

* In main models file `<model>.rs` we define model, initialization logic (if needed) and `Read` related methods
* We use `<model>/<operation>.rs` to define model's operation related logic e.g. `create`, `update`, `delete`
* In `<model>/callbacks.rs` we define model's callbacks implementation where we trigger associated operation logic
  e.g. `before_create`, `after_create`, `before_update`, `after_update`, `before_delete`, `after_delete`
* We usually use `before_` callbacks to set defaults and validate data while `after_` callbacks are used to trigger
  associated business logic related to the operation. We usually try to wrap after callbacks in a async thread to
  avoid blocking the main thread.
* In `<models>/<partial_model>` we define logic for partial model that's not related to 'read` methods.
* In `traits/` directory we define and implement traits for models

### Exceptions

* For models that don't have partial logic, we define all the logic in main model file and don't create
  `<model>/<operation>.rs` and `<model>/callbacks.rs` files
