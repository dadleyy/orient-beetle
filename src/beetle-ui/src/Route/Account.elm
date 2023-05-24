module Route.Account exposing (Message(..), Model, default, update, view)

import Button
import Environment
import Html
import Html.Attributes as A
import Icon


type alias Model =
    { session : Environment.Session
    }


type Message
    = Tick


default : Environment.Environment -> ( Maybe Model, Cmd Message )
default env =
    let
        maybeSession =
            Environment.getSession env

        maybeModel =
            maybeSession
                |> Maybe.map (\session -> { session = session })
    in
    ( maybeModel, Cmd.none )


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    ( model, Cmd.none )


view : Model -> Environment.Environment -> Html.Html Message
view model env =
    Html.div [ A.class "mt-4 mx-10 flex flex-start" ]
        [ Html.div [ A.class "mx-auto mb-4 rounded overflow-hidden" ]
            [ Html.img [ A.src model.session.picture ] [] ]
        , Html.div [ A.class "flex-1 ml-4" ]
            [ Html.table [ A.class "w-full info-table" ]
                [ Html.tbody []
                    [ Html.tr []
                        [ Html.td [] [ Html.text "ID:" ]
                        , Html.td [] [ Html.code [] [ Html.text model.session.oid ] ]
                        , Html.td [ A.class "opacity-0" ]
                            [ Button.view (Button.DisabledIcon Icon.Pencil) ]
                        ]
                    ]
                , Html.tr []
                    [ Html.td [] [ Html.text "Nickname:" ]
                    , Html.td [] [ Html.text "" ]
                    , Html.td [ A.class "text-right" ]
                        [ Button.view (Button.SecondaryIcon Icon.Pencil Tick) ]
                    ]
                ]
            ]
        ]
