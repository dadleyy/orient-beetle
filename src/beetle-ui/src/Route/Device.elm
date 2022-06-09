module Route.Device exposing (Message(..), Model, default, update, view)

import Environment
import Html
import Html.Attributes
import Http


type Message
    = Loaded (Result Http.Error ())
    | Sent


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


fetchDevice : Environment.Environment -> String -> Cmd Message
fetchDevice env id =
    Http.get
        { url = Environment.apiRoute env ("device-info?id=" ++ id)
        , expect = Http.expectWhatever Loaded
        }


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    ( model, Cmd.none )


default : Environment.Environment -> String -> ( Model, Cmd Message )
default env id =
    ( { id = id }, Cmd.batch [ fetchDevice env id ] )
