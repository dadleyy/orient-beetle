module Route.Home exposing (Model)

import Http

type alias Data = 
  {
    devices : List String
  }

type Model = Result Http.Error Data
