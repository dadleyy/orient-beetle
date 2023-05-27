module Dropdown exposing (Dropdown, empty, subscriptions, view)

import Browser.Events as Browser
import Button
import Html
import Html.Attributes as A
import Html.Events as E
import Icon
import Json.Decode as D


type alias Dropdown =
    { opened : Bool

    -- the `workaround` here is to get around a strange unknown issue with the subscription that will
    -- immediately pass the `onClick` cmd created when opened, closing the dropdown immediately.
    , workaround : Int
    }


empty : Dropdown
empty =
    { opened = False, workaround = 0 }


subscriptions : (Dropdown -> Maybe b -> a) -> Dropdown -> Sub a
subscriptions events drop =
    let
        messageCons =
            case drop.workaround of
                0 ->
                    events { drop | workaround = 1 } Nothing

                _ ->
                    events { drop | opened = False, workaround = 0 } Nothing
    in
    if drop.opened then
        Browser.onClick (D.succeed messageCons)

    else
        Sub.none


view : Dropdown -> (Dropdown -> Maybe b -> a) -> List ( b, Html.Html a ) -> Html.Html a
view drop onOpen elements =
    let
        nextState =
            { drop | opened = not drop.opened, workaround = 0 }
    in
    Html.div [ A.class "relative" ]
        [ Html.div [ A.class "relative" ]
            [ Button.view (Button.SecondaryIcon Icon.EllipsisH (onOpen nextState Nothing)) ]
        , Html.div
            [ if drop.opened then
                A.class "block absolute rounded overflow-hidden right-0 top-full z-40"

              else
                A.class "hidden"
            ]
            (List.map (wrapOption onOpen) elements)
        ]


wrapOption : (Dropdown -> Maybe b -> a) -> ( b, Html.Html a ) -> Html.Html a
wrapOption updater content =
    Html.div
        [ E.onClick (updater { opened = False, workaround = 0 } (Just (Tuple.first content)))
        , A.class "dropdown-item"
        ]
        [ Tuple.second content ]
