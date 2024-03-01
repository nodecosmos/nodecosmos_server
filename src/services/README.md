### For now:

* Services are used only for the logic that is external to the application like: aws, redis, elastic.

### TODO:

* Move them to traits in models as even services are part of the application logic that is model specific.
  e.g. have S3 trait in models/traits and we can implement it with models or partials that need it.
  In case model has multiple s3 related fields .e.g cover_image, profile_image we can have a separate
  trait for partial_<model_name> and implement S3 trait in it.
