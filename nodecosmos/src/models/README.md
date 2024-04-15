### Models

* In main models file `<model>.rs` we define model, `Read` related methods and we do `Callbacks`
  implementation.
* We usually use `before_` callbacks to set defaults and validate data while `after_` callbacks are
  used to trigger
  associated business logic related to the operation. We usually try to wrap after callbacks in a
  async thread to
  avoid blocking the main thread.
* We use `<model>/<operation>.rs` to define model's operation related logic
  e.g. `create`, `update`, `delete`
* In `<models>/<partial_model>` we define logic for partial model that's not related to 'read`
  methods.
* In `traits/` directory we define and implement traits for models
* in `traits/<model>` we define model's traits that are implemented by model and it's partial models
* in `nodecosmos-macros` we define derives for models

### Exceptions

* For simple models that don't have partials, we define all the logic in main model file and don't
  create
  `<model>/<operation>.rs` file.
