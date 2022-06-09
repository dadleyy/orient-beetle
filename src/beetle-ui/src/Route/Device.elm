module Route.Device exposing (Model, default, view)

import Environment
import Html
import Html.Attributes


type alias Model =
    { id : String
    }


view : Model -> Environment.Environment -> Html.Html ()
view model env =
    Html.div
        [ Html.Attributes.class "px-4 py-3"
        ]
        [ Html.div [ Html.Attributes.class "pb-1 mb-1" ] [ Html.h2 [] [ Html.text model.id ] ]
        , Html.div [ Html.Attributes.class "flex items-center" ]
            [ Html.input [] []
            , Html.button [] [ Html.text "send" ]
            ]
        ]


default : Environment.Environment -> String -> Model
default env id =
    { id = id }
